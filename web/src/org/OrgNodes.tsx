import { Link } from '@tanstack/react-router'
import { useI18n } from '../i18n'
import { Face } from '../people/Face'
import type { AgentNode, DepartmentNode, PersonNode, TeamNode } from './tree'

export function DepartmentCard({ node, org }: { node: DepartmentNode; org: string }) {
  const { t } = useI18n()
  return (
    <Link className="oc-node department" to="/org" search={{ org, unit: node.department.id }}>
      <span className="oc-kind">{t.org.kinds.department}</span>
      <span className="oc-name">{node.department.label}</span>
      <span className="oc-sub">{node.head === undefined ? t.org.noHead : t.org.ledBy(node.head.label)}</span>
      {node.waiting > 0 ? <span className="oc-wait">{t.org.waitingOnYou(node.waiting)}</span> : null}
    </Link>
  )
}

export function TeamCard({ node }: { node: TeamNode }) {
  const { t } = useI18n()
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
