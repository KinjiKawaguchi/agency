import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../i18n'
import { edgesOf, keyOf, type Agent, type Edge, type Point, type Resolve } from './graph'
import type { TaggedEvent } from './stream'
import { useForceLayout } from './useForceLayout'
import { useTicker } from './useTicker'
import { initials } from '../people/identity'

/** How long a packet takes from one agent to the other, and how long a line and a point stay lit after a message. */
const PACKET_MS = 900
const LIT_MS = 2_600

interface Props {
  /** The agents of the teams, and the guests that messages name. */
  agents: readonly Agent[]
  /** Who a name in a message is. */
  resolve: Resolve
  /** What happened lately: the lines come from its messages. */
  events: readonly TaggedEvent[]
  /** What has just arrived: each message in it sends a packet along its line. */
  fresh: readonly TaggedEvent[]
  isReduced: boolean
  /** The color of each team, by `org/team`. */
  colors: ReadonlyMap<string, string>
}

interface Packet {
  id: string
  from: string
  to: string
  startedAt: number
}

/** A name as it is written under its point: long ones are cut (the whole name is in the point's title). */
const NAME_CHARS = 24
const shortName = (name: string): string => (name.length > NAME_CHARS ? `${name.slice(0, NAME_CHARS - 1)}…` : name)

const edgeId = (edge: { a: string; b: string }): string => `${edge.a}|${edge.b}`
const lerp = (from: number, to: number, t: number): number => from + (to - from) * t

/**
 * Who writes to whom: agents are points and a line joins two that have written to each other lately, thicker for
 * the busier. A message that has just passed sends a packet along its line and lights it, and the two points
 * glow. Where the viewer asked for less motion nothing travels: the line and the points light up and fade.
 */
export function GraphView({ agents, resolve, events, fresh, isReduced, colors }: Props) {
  const { t } = useI18n()
  const box = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ width: 760, height: 480 })
  const packets = useRef<Packet[]>([])
  // The latest resolver, read when a message arrives: it must not make the effect run again for events it has seen.
  const resolveRef = useRef(resolve)
  resolveRef.current = resolve
  const litAt = useRef(new Map<string, number>())
  // The messages already acted on: a message stays in `fresh` for a few seconds, and must send its packet once.
  const acted = useRef(new Set<string>())
  const [clock, setClock] = useState(() => Date.now())

  useEffect(() => {
    const element = box.current
    if (element === null || typeof ResizeObserver === 'undefined') return undefined
    const measure = (): void => {
      const width = Math.max(280, Math.round(element.clientWidth))
      setSize({ width, height: Math.round(Math.min(560, Math.max(320, width * 0.62))) })
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  const edges: Edge[] = edgesOf(events, { now: clock, resolve })
  const { points, isMoving } = useForceLayout(agents, edges, size, isReduced)

  // A message that has just arrived lights its line and its two points, and sends a packet unless motion is not wanted.
  useEffect(() => {
    const now = Date.now()
    for (const { org, team, event } of fresh) {
      if (event.kind !== 'message' || acted.current.has(`${org}/${team}/${event.id}`)) continue
      acted.current.add(`${org}/${team}/${event.id}`)
      const [from, to] = [resolveRef.current(org, team, event.from /* shown-ok(lookup): finds the agent drawn */), resolveRef.current(org, team, event.to /* shown-ok(lookup): as above */)]
      for (const lit of [from, to, edgeId({ a: from < to ? from : to, b: from < to ? to : from })]) litAt.current.set(lit, now)
      if (!isReduced) packets.current = [...packets.current.filter((packet) => now - packet.startedAt < PACKET_MS), { id: event.id, from, to, startedAt: now }]
    }
  }, [fresh, isReduced])

  const hasGlow = [...litAt.current.values()].some((at) => clock - at < LIT_MS)
  const hasPackets = packets.current.some((packet) => clock - packet.startedAt < PACKET_MS)
  // Every frame while something travels or moves; a slow beat while something fades; and a slower one for the weights' decay.
  const tick = useTicker(true, !isReduced && (isMoving || hasPackets) ? 'frame' : hasGlow ? 250 : 5_000)
  useEffect(() => setClock(tick), [tick])

  const place = new Map<string, Point>(points.map((point) => [point.key, point]))
  const glowOf = (key: string): number => Math.max(0, 1 - (clock - (litAt.current.get(key) ?? -Infinity)) / LIT_MS)

  return (
    <div ref={box} className="lv-graph">
      <svg viewBox={`0 0 ${size.width} ${size.height}`} width={size.width} height={size.height} role="img" aria-label={t.live.graph.label}>
        <g className="lv-edges">
          {edges.map((edge) => {
            const [p, q] = [place.get(edge.a), place.get(edge.b)]
            if (p === undefined || q === undefined) return null
            const glow = glowOf(edgeId(edge))
            return (
              <g key={edgeId(edge)}>
                <line className="lv-edge" x1={p.x} y1={p.y} x2={q.x} y2={q.y} strokeWidth={1 + Math.min(edge.weight, 4) * 1.2} style={{ opacity: 0.25 + Math.min(edge.weight, 3) * 0.15 }} />
                {glow > 0 ? <line className="lv-edge-lit" x1={p.x} y1={p.y} x2={q.x} y2={q.y} strokeWidth={3 + glow * 4} style={{ opacity: glow }} /> : null}
              </g>
            )
          })}
        </g>
        <g className="lv-packets" aria-hidden="true">
          {packets.current.map((packet) => {
            const [p, q] = [place.get(packet.from /* shown-ok(lookup): finds the point drawn */), place.get(packet.to /* shown-ok(lookup): as above */)]
            const progress = (clock - packet.startedAt) / PACKET_MS
            if (p === undefined || q === undefined || progress < 0 || progress > 1) return null
            return <circle key={packet.id} className="lv-packet" cx={lerp(p.x, q.x, progress)} cy={lerp(p.y, q.y, progress)} r={5} />
          })}
        </g>
        <g className="lv-nodes">
          {agents.map((agent) => {
            const point = place.get(keyOf(agent))
            if (point === undefined) return null
            const glow = glowOf(keyOf(agent))
            const color = colors.get(`${agent.org}/${agent.team}`) ?? 'var(--series-other)'
            return (
              <g key={keyOf(agent)} className="lv-node" transform={`translate(${point.x} ${point.y})`}>
                <title>{t.live.graph.agent(agent.label, agent.team)}</title>
                {glow > 0 ? <circle className="lv-halo" r={16 + (1 - glow) * 14} style={{ opacity: glow * 0.8, stroke: color }} /> : null}
                <circle className="lv-dot" r={15} style={{ fill: color }} />
                <text className="lv-initials" textAnchor="middle" dy="0.35em">{initials(agent.label)}</text>
                <text className="lv-name" textAnchor="middle" y={30}>{shortName(agent.label)}</text>
              </g>
            )
          })}
        </g>
      </svg>
    </div>
  )
}
