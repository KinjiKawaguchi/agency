import { useEffect, useRef, useState } from 'react'
import { type Agent, type Edge, type Point, type Size, keyOf, placeAgents, REST_ENERGY, settle, step } from './graph'


/** How much energy a change gives the layout, to move the points to their new places. */
const REHEAT = 0.7
const COOLING = 0.975

/**
 * The points of the agents, moving to where the forces put them. Where the viewer asked for less motion they
 * are put there at once and nothing moves; otherwise they glide, and the frames stop when they are at rest.
 */
export function useForceLayout(agents: readonly Agent[], edges: readonly Edge[], size: Size, isReduced: boolean): { points: Point[]; isMoving: boolean } {
  const latest = useRef({ agents, edges, size })
  latest.current = { agents, edges, size }
  const [points, setPoints] = useState<Point[]>(() => settle(placeAgents(agents, [], size), { agents, edges, size }, 120))
  const [isMoving, setIsMoving] = useState(false)
  const energy = useRef(0)
  // What the forces are made of, as a text: the layout moves when this changes, not for every tick of the weights fading.
  const signature = `${agents.map(keyOf).join(',')}|${edges.map((edge) => `${edge.a}-${edge.b}:${Math.round(edge.weight * 2)}`).join(',')}|${Math.round(size.width)}x${Math.round(size.height)}`

  useEffect(() => {
    if (isReduced) {
      setPoints((previous) => settle(placeAgents(latest.current.agents, previous, latest.current.size), latest.current))
      return
    }
    setPoints((previous) => placeAgents(latest.current.agents, previous, latest.current.size))
    energy.current = REHEAT
    setIsMoving(true)
  }, [signature, isReduced])

  useEffect(() => {
    if (!isMoving || isReduced) return undefined
    let id = 0
    const frame = (): void => {
      setPoints((previous) => step(previous, latest.current, energy.current))
      energy.current *= COOLING
      if (energy.current <= REST_ENERGY) setIsMoving(false)
      else id = window.requestAnimationFrame(frame)
    }
    id = window.requestAnimationFrame(frame)
    return () => window.cancelAnimationFrame(id)
  }, [isMoving, isReduced])

  return { points, isMoving }
}
