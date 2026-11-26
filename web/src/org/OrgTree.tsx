import { useI18n } from '../i18n'
import { AgentCard, DepartmentCard, OrgCard, PersonCard, TeamCard } from './OrgNodes'
import type { OrgNode, OrgTree as Tree, TeamNode, UnitChild } from './tree'

/** The agents of a team in one row under it, the lead first. */
function Agents({ node }: { node: TeamNode }) {
  if (node.agents.length === 0) return null
  const agents = [...node.agents.filter((agent) => agent.isLead), ...node.agents.filter((agent) => !agent.isLead)]
  return (
    <ul>
      {agents.map((agent) => (
        <li key={agent.person.id}>
          <AgentCard agent={agent} />
        </li>
      ))}
    </ul>
  )
}

const keyOf = (child: UnitChild): string =>
  `${child.kind}:${child.kind === 'department' ? child.department.id : child.kind === 'team' ? child.team.id : child.kind === 'org' ? child.org.id : child.person.id}`

/** The units under a box, in a row. `compact` draws the chart of every org: a team's participants are faces in its box. */
function Branch({ children, org, compact }: { children: readonly UnitChild[]; org: string; compact: boolean }) {
  if (children.length === 0) return null
  return (
    <ul>
      {children.map((child) => (
        <li key={keyOf(child)}>
          {child.kind === 'org' ? (
            <>
              <OrgCard node={child} />
              <Branch children={child.children} org={child.org.id} compact={compact} />
            </>
          ) : child.kind === 'department' ? (
            <>
              <DepartmentCard node={child} org={org} />
              <Branch children={child.children} org={org} compact={compact} />
            </>
          ) : child.kind === 'team' ? (
            <>
              <TeamCard node={child} compact={compact} />
              {compact ? null : <Agents node={child} />}
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
          <Branch children={tree.children} org={org} compact={false} />
        </li>
      </ul>
    </div>
  )
}

/** Every org of the account side by side under the account, each with its departments, teams, and people. */
export function AllOrgsTree({ account, orgs }: { account: string; orgs: readonly OrgNode[] }) {
  const { t } = useI18n()
  return (
    <div className="orgchart all" role="group" aria-label={t.org.allChartLabel}>
      <ul>
        <li>
          <div className="oc-node person top is-you">
            <span className="oc-kind">{t.org.kinds.account}</span>
            <span className="oc-name">{account}</span>
          </div>
          <Branch children={orgs} org="" compact />
        </li>
      </ul>
    </div>
  )
}
