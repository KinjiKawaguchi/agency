import type { OrgDefinition, Department, Person, TeamPlace } from '../api/clientApi'
import type { Team } from '../api/teams'
import type { PersonStatus, TeamView } from '../api/teamview'
import { presenceOf, type Presence } from '../people/state'

export interface AgentNode {
  person: PersonStatus
  presence: Presence
  isLead: boolean
}

export interface TeamNode {
  kind: 'team'
  team: TeamPlace
  /** Absent when the node could not read the team. */
  view: TeamView | undefined
  agents: AgentNode[]
  working: number
  waiting: number
}

export interface PersonNode {
  kind: 'person'
  person: Person
  isYou: boolean
}

export interface DepartmentNode {
  kind: 'department'
  department: Department
  head: Person | undefined
  children: UnitChild[]
  /** Agents waiting for the viewer anywhere below. */
  waiting: number
}

export type UnitChild = DepartmentNode | TeamNode | PersonNode

export interface OrgTree {
  /** The department the chart is focused on, or `undefined` for the whole organization. */
  focus: DepartmentNode | undefined
  /** What hangs directly under the top box. */
  children: UnitChild[]
  waiting: number
}

function teamNode(place: TeamPlace, teams: readonly Team[], orgId: string): TeamNode {
  const found = teams.find((one) => one.view.org === orgId && one.view.team === place.id)
  if (found === undefined) return { kind: 'team', team: place, view: undefined, agents: [], working: 0, waiting: 0 }
  const { view, viewer } = found
  const agents = view.people
    .filter((person) => person.kind === 'agent')
    .map((person) => ({ person, presence: presenceOf(person, viewer), isLead: person.role === 'lead' }))
  return {
    kind: 'team',
    team: place,
    view,
    agents,
    working: agents.filter((agent) => agent.person.state === 'working').length,
    waiting: agents.filter((agent) => agent.presence === 'needs-you').length,
  }
}

const waitingOf = (children: readonly UnitChild[]): number =>
  children.reduce((sum, child) => sum + (child.kind === 'person' ? 0 : child.waiting), 0)

/** A department's departments, teams, and people, in that order. `null` is the top of the organization. */
function childrenOf(parent: string | null, definition: OrgDefinition, teams: readonly Team[], viewerAccount: string): UnitChild[] {
  const departments = definition.departments
    .filter((department) => department.parent === parent)
    .map((department): DepartmentNode => {
      const children = childrenOf(department.id, definition, teams, viewerAccount)
      return { kind: 'department', department, head: definition.people.find((person) => person.id === department.head), children, waiting: waitingOf(children) }
    })
  const places = definition.teams.filter((place) => place.department === parent).map((place) => teamNode(place, teams, definition.org))
  const people = definition.people
    .filter((person) => person.department === parent)
    .map((person): PersonNode => ({ kind: 'person', person, isYou: person.account === viewerAccount }))
  return [...departments, ...places, ...people]
}

function findDepartment(children: readonly UnitChild[], id: string): DepartmentNode | undefined {
  for (const child of children) {
    if (child.kind !== 'department') continue
    if (child.department.id === id) return child
    const inside = findDepartment(child.children, id)
    if (inside !== undefined) return inside
  }
  return undefined
}

/** The chart of an organization, optionally focused on one department and everything below it. */
export function buildOrgTree(definition: OrgDefinition, teams: readonly Team[], viewerAccount: string, focusId?: string): OrgTree {
  const whole = childrenOf(null, definition, teams, viewerAccount)
  const focus = focusId === undefined ? undefined : findDepartment(whole, focusId)
  const children = focus === undefined ? whole : focus.children
  return { focus, children, waiting: waitingOf(children) }
}
