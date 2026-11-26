import { useMemo } from 'react'
import { useSuspenseTeamStates } from '../api/queries'
import { useI18n } from '../i18n'
import { Region } from '../shared/Region'
import { Board } from './Board'
import { Feed } from './Feed'
import { GraphView } from './GraphView'
import { guestsOf, resolverOf, type Agent } from './graph'
import { LiveSkeleton } from './LiveSkeleton'
import { Throughput } from './Throughput'
import { useActivityStream } from './useActivityStream'
import { useReducedMotion } from './useReducedMotion'
import { useTicker } from './useTicker'
import './live.css'

/** A page to leave open: the department moving. Experimental, and rough on purpose. */
export function LivePage() {
  const { t } = useI18n()
  return (
    <>
      <div className="page-head">
        <h1 className="page-title">{t.live.title}</h1>
        <span className="lv-chip">{t.live.experimental}</span>
      </div>
      <p className="muted lv-lead">{t.live.lead}</p>
      <Region what={t.live.title} skeleton={<LiveSkeleton />}>
        <Live />
      </Region>
    </>
  )
}

function Live() {
  const { t } = useI18n()
  const { orgs, teams } = useSuspenseTeamStates()
  const refs = orgs.flatMap(({ org, teams: inOrg }) => inOrg.map(({ ref }) => ({ org: org.id, team: ref.id })))
  const stream = useActivityStream(refs)
  const isReduced = useReducedMotion()
  const now = useTicker(true, 5_000)

  const agents: Agent[] = teams.flatMap((team) =>
    team.view.people.filter((person) => person.kind === 'agent').map((person) => ({ id: person.id, label: person.label, team: team.view.team, org: team.view.org })),
  )
  const resolve = useMemo(() => resolverOf(agents), [agents])
  const { shown: guests, total: guestTotal } = guestsOf(stream.events, resolve, { now })
  const teamKeys = [...new Set(refs.map((ref) => `${ref.org}/${ref.team}`))].toSorted()
  const colors = new Map(teamKeys.map((key, index) => [key, `var(--series-${(index % 8) + 1})`]))
  const teamLabels = new Map<string, string>(orgs.flatMap(({ org, teams: inOrg }) => inOrg.map(({ ref }) => [`${org.id}/${ref.id}`, ref.label] as [string, string])))
  const readable = refs.length - stream.unreported.length
  const delayed = stream.failing.map((key) => teamLabels.get(key) ?? key)

  return (
    <div className="lv-grid">
      <Throughput events={stream.events} fresh={stream.fresh} orgs={orgs.map(({ org }) => org.id)} now={now} isReading={stream.pending.length > 0} hasUnread={stream.neverRead.length > 0} />
      <section className="lv-panel lv-graph-panel" aria-label={t.live.graph.title}>
        <header className="lv-panel-head">
          <h2>{t.live.graph.title}</h2>
          <span className="muted">{t.live.graph.hint}</span>
        </header>
        {agents.length === 0 ? <p className="muted lv-empty">{t.live.noAgents}</p> : <GraphView agents={[...agents, ...guests]} resolve={resolve} events={stream.events} fresh={stream.fresh} isReduced={isReduced} colors={colors} />}
        <ul className="lv-legend">
          {teamKeys.map((key) => (
            <li key={key}>
              <i style={{ background: colors.get(key) }} />
              {teamLabels.get(key) ?? key}
            </li>
          ))}
          {guests.length === 0 ? null : (
            <li>
              <i style={{ background: 'var(--series-other)' }} />
              {guestTotal > guests.length ? t.live.graph.othersMore(guests.length, guestTotal) : t.live.graph.others}
            </li>
          )}
        </ul>
      </section>
      <section className="lv-panel lv-feed-panel" aria-label={t.live.feed.title}>
        <header className="lv-panel-head">
          <h2>{t.live.feed.title}</h2>
        </header>
        {refs.length > 0 && readable === 0 ? <p className="muted">{t.live.notReported}</p> : <Feed events={stream.events} fresh={isReduced ? [] : stream.fresh} teamLabels={teamLabels} isReading={stream.pending.length > 0} hasUnread={stream.neverRead.length > 0} />}
        {delayed.length === 0 ? null : <p className="muted lv-late" role="status">{t.live.delayed(delayed.join(', '))}</p>}
      </section>
      <section className="lv-panel lv-board-panel" aria-label={t.live.board.title}>
        <header className="lv-panel-head">
          <h2>{t.live.board.title}</h2>
        </header>
        <Board teams={teams} />
      </section>
    </div>
  )
}
