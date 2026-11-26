import { Link } from '@tanstack/react-router'
import { useI18n } from '../i18n'
import { Face } from '../people/Face'
import type { AgentNode, DepartmentNode, OrgNode, PersonNode, TeamNode } from './tree'

/** How many faces a compact team shows before saying how many more there are. */
const FACES = 6

export function DepartmentCard({ node, org, compact = false }: { node: DepartmentNode; org: string; compact?: boolean }) {
  const { t } = useI18n()
  const waiting = compact ? node.mine : node.waiting
  return (
    <Link className="oc-node department" to="/org" search={{ org, unit: node.department.id }}>
      <span className="oc-kind">{t.org.kinds.department}</span>
      <span className="oc-name">{node.department.label}</span>
      <span className="oc-sub">{node.head === undefined ? t.org.noHead : t.org.ledBy(node.head.label)}</span>
      {waiting > 0 ? <span className="oc-wait">{t.org.waitingOnYou(waiting)}</span> : null}
    </Link>
  )
}

/** A team of the chart of every org: its signals (waiting on the viewer, stuck, working) and its participants as faces. */
function CompactTeam({ node }: { node: TeamNode }) {
  const { t } = useI18n()
  const { signals } = node
  const shown = node.agents.slice(0, FACES)
  return (
    <Link className={`oc-node team compact${signals === undefined ? '' : ` attention-${signals.attention}`}`} to="/overview">
      <span className="oc-kind">{t.org.kinds.team}</span>
      <span className="oc-name">{node.team.label}</span>
      {signals === undefined ? (
        <span className="oc-wait">{t.org.teamUnavailable}</span>
      ) : (
        <>
          {signals.mine > 0 ? <span className="oc-wait">{t.org.waitingOnYou(signals.mine)}</span> : null}
          {signals.attention === 'stuck' ? <span className="oc-stuck">{t.overview.tags.stuck}</span> : null}
          {signals.attention === 'idle' ? <span className="oc-stuck">{t.overview.tags.idle}</span> : null}
          <span className="oc-sub">{signals.participants === 0 ? t.overview.noParticipants : t.overview.working(signals.working, signals.participants)}</span>
        </>
      )}
      {shown.length === 0 ? null : (
        <span className="oc-faces">
          {shown.map((agent) => (
            <Face key={agent.person.id} label={agent.person.label} kind="agent" state={agent.presence} size="xs" title={`${agent.person.label}: ${agent.person.status}`} />
          ))}
          {node.agents.length > FACES ? <span className="oc-more">+{node.agents.length - FACES}</span> : null}
        </span>
      )}
    </Link>
  )
}

export function TeamCard({ node, compact = false }: { node: TeamNode; compact?: boolean }) {
  const { t } = useI18n()
  if (compact) return <CompactTeam node={node} />
  return (
    <Link className="oc-node team" to="/people">
      <span className="oc-kind">{t.org.kinds.team}</span>
      <span className="oc-name">{node.team.label}</span>
      {node.view === undefined ? (
        <span className="oc-wait">{t.org.teamUnavailable}</span>
      ) : (
        <span className="oc-sub">{t.org.agents(node.agents.length, node.working)}</span>
      )}
      {node.waiting > 0 ? <span className="oc-wait">{t.org.waitingOnYou(node.waiting)}</span> : null}
    </Link>
  )
}

export function PersonCard({ node }: { node: PersonNode }) {
  const { t } = useI18n()
  const notes = [node.isYou ? t.org.you : null, node.person.role === 'answerer' ? t.org.answerer : null].filter((note) => note !== null)
  return (
    <div className={`oc-node person${node.isYou ? ' is-you' : ''}`}>
      <Face label={node.person.label} kind="human" size="md" />
      <span className="oc-name">{node.person.label}</span>
      <span className="oc-sub">{notes.length === 0 ? t.org.kinds.member : notes.join(', ')}</span>
    </div>
  )
}

export function AgentCard({ agent }: { agent: AgentNode }) {
  const { t } = useI18n()
  return (
    <Link className={`oc-node agent presence-${agent.presence}`} to="/people" title={agent.person.status}>
      <Face label={agent.person.label} kind="agent" state={agent.presence} size="md" />
      <span className="oc-name">{agent.person.label}</span>
      <span className="oc-kind">{agent.isLead ? t.org.kinds.lead : t.org.kinds.agent}</span>
      <span className="oc-now">{agent.presence === 'needs-you' ? t.org.yourTurn : agent.person.status}</span>
    </Link>
  )
}

/** An org of the chart of every org; opens that org's own chart. */
export function OrgCard({ node }: { node: OrgNode }) {
  const { t } = useI18n()
  return (
    <Link className="oc-node org" to="/org" search={{ org: node.org.id }}>
      <span className="oc-kind">{t.org.kinds.organization}</span>
      <span className="oc-name">{node.org.label}</span>
      {node.state === 'failed' ? <span className="oc-wait">{t.org.orgUnavailable}</span> : null}
      {node.state === 'loading' ? <span className="oc-sub">{t.app.loading}</span> : null}
      {node.mine > 0 ? <span className="oc-wait">{t.org.waitingOnYou(node.mine)}</span> : null}
    </Link>
  )
}
