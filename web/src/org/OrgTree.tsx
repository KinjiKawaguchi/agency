import { useI18n } from '../i18n'
import { AgentCard, DepartmentCard, PersonCard, TeamCard } from './OrgNodes'
import type { OrgTree as Tree, TeamNode, UnitChild } from './tree'

/** The agents of a team: the lead first, the others in a column beneath. */
function Agents({ node }: { node: TeamNode }) {
  if (node.agents.length === 0) return null
  const lead = node.agents.find((agent) => agent.isLead)
  const members = node.agents.filter((agent) => agent !== lead)
  return (
    <ul className="oc-agents">
      {lead === undefined ? null : (
        <li>
          <AgentCard agent={lead} />
        </li>
      )}
      {members.map((agent) => (
        <li key={agent.person.id} className={lead === undefined ? undefined : 'member'}>
          <AgentCard agent={agent} />
        </li>
      ))}
    </ul>
  )
}

function Branch({ children, org }: { children: readonly UnitChild[]; org: string }) {
  if (children.length === 0) return null
  return (
    <ul>
      {children.map((child) => (
        <li key={`${child.kind}:${child.kind === 'department' ? child.department.id : child.kind === 'team' ? child.team.id : child.person.id}`}>
          {child.kind === 'department' ? (
            <>
              <DepartmentCard node={child} org={org} />
              <Branch children={child.children} org={org} />
            </>
          ) : child.kind === 'team' ? (
            <>
              <TeamCard node={child} />
              <Agents node={child} />
            </>
          ) : (
            <PersonCard node={child} />
          )}
        </li>
      ))}
    </ul>
  )
}

interface OrgTreeProps {
  tree: Tree
  org: string
  orgLabel: string
}

/** The top box (the organization, or the focused department) with everything below it. */
export function OrgTree({ tree, org, orgLabel }: OrgTreeProps) {
  const { t } = useI18n()
  const top = tree.focus === undefined ? { kind: t.org.kinds.organization, label: orgLabel, sub: null } : { kind: t.org.kinds.department, label: tree.focus.department.label, sub: tree.focus.head === undefined ? t.org.noHead : t.org.ledBy(tree.focus.head.label) }
  return (
    <div className="orgchart" role="group" aria-label={t.org.chartLabel}>
      <ul>
        <li>
          <div className="oc-node department top">
            <span className="oc-kind">{top.kind}</span>
            <span className="oc-name">{top.label}</span>
            {top.sub === null ? null : <span className="oc-sub">{top.sub}</span>}
          </div>
          <Branch children={tree.children} org={org} />
        </li>
      </ul>
    </div>
  )
}
