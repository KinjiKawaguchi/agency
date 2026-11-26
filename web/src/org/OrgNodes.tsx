import { Link } from '@tanstack/react-router'
import { useI18n } from '../i18n'
import { Face } from '../people/Face'
import { Skeleton } from '../shared/Skeleton'
import type { Team } from '../api/teams'
import type { AgentNode, DepartmentNode, Mine, OrgNode, PersonNode, TeamNode } from './tree'

/** The requests that are the viewer's, in the overview's words: the same number in every chart. */
function Waiting({ count }: { count: Mine }) {
  const { t } = useI18n()
  if (count === 'loading') return <Skeleton kind="short" />
  if (count === 'unknown') return <span className="oc-wait oc-unread">{t.org.someUnread}</span>
  return typeof count === 'number' && count > 0 ? <span className="oc-wait">{t.overview.waitingForYou(count)}</span> : null
}

/** A mark on a team whose data is a copy that could not be renewed; the age and the reason are in its title, as in the overview. */
function OldData({ source }: { source: Team['source'] | undefined }) {
  const { t, fmt } = useI18n()
  if (source === undefined || source.kind !== 'replica' || source.error === undefined) return null
  const title = source.fetchedAt === undefined ? t.overview.neverRead(source.error) : t.overview.staleFrom(fmt.relative(source.fetchedAt, t.time.justNow), source.error)
  return (
    <span className="oc-old" title={title}>
      {t.org.oldData}
    </span>
  )
}

/** How many faces a compact team shows before saying how many more there are. */
const FACES = 6

export function DepartmentCard({ node, org }: { node: DepartmentNode; org: string }) {
  const { t } = useI18n()
  return (
    <Link className="oc-node department" to="/org" search={{ org, unit: node.department.id }}>
      <span className="oc-kind">{t.org.kinds.department}</span>
      <span className="oc-name">{node.department.label}</span>
      <span className="oc-sub">{node.head === undefined ? t.org.noHead : t.org.ledBy(node.head.label)}</span>
      <Waiting count={node.mine} />
    </Link>
  )
}

/** A team of the chart of every org: its signals (waiting on the viewer, stuck, working) and its participants as faces. */
function CompactTeam({ node }: { node: TeamNode }) {
  const { t } = useI18n()
  const { signals } = node
  const shown = node.agents.slice(0, FACES)
  return (
    <Link className={`oc-node team compact${signals === undefined ? '' : ` attention-${signals.attention}`}`} {...destination(node)}>
      <span className="oc-kind">{t.org.kinds.team}</span>
      <span className="oc-name">{node.team.label}</span>
      <OldData source={node.source} />
      {node.isLoading ? (
        <Skeleton kind="short" />
      ) : signals === undefined ? (
        <span className="oc-wait">{t.org.teamUnavailable}</span>
      ) : (
        <>
          <Waiting count={signals.mine} />
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

/** Where a team's box goes: its requests on the desk when the viewer has some, else the overview. */
function destination(node: TeamNode): { to: '/desk'; search: { team: string } } | { to: '/overview' } {
  return node.view !== undefined && node.signals !== undefined && node.signals.mine > 0 ? { to: '/desk', search: { team: `${node.view.org}/${node.view.team}` } } : { to: '/overview' }
}

export function TeamCard({ node, compact = false }: { node: TeamNode; compact?: boolean }) {
  const { t } = useI18n()
  if (compact) return <CompactTeam node={node} />
  return (
    <Link className="oc-node team" to="/people">
      <span className="oc-kind">{t.org.kinds.team}</span>
      <span className="oc-name">{node.team.label}</span>
      {node.view === undefined ? (
        node.isLoading ? <Skeleton kind="short" /> : <span className="oc-wait">{t.org.teamUnavailable}</span>
      ) : (
        <span className="oc-sub">{t.org.agents(node.agents.length, node.working)}</span>
      )}
      <OldData source={node.source} />
      {node.signals === undefined ? null : <Waiting count={node.signals.mine} />}
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
      <Waiting count={node.mine} />
    </Link>
  )
}
