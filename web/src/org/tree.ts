import type { OrgDefinition, OrgRef, Department, Person, TeamPlace } from '../api/clientApi'
import type { Team } from '../api/teams'
import type { PersonStatus, TeamView } from '../api/teamview'
import { summarize, type TeamSummary } from '../overview/summary'
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
  /** What the overview says of the team: requests waiting on the viewer, blocked work, who is working. Absent when the team could not be read. */
  signals: TeamSummary | undefined
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
  /** Requests that are the viewer's anywhere below. */
  mine: number
}

/** An org in the chart of every org: what the node says of it, and its units. */
export interface OrgNode {
  kind: 'org'
  org: OrgRef
  /** Absent while the org's definition is loading or when it could not be read. */
  children: UnitChild[]
  mine: number
  state: 'ready' | 'loading' | 'failed'
}

export type UnitChild = DepartmentNode | TeamNode | PersonNode | OrgNode

export interface OrgTree {
  /** The department the chart is focused on, or `undefined` for the whole organization. */
  focus: DepartmentNode | undefined
  /** What hangs directly under the top box. */
  children: UnitChild[]
  waiting: number
}

function teamNode(place: TeamPlace, teams: readonly Team[], orgId: string): TeamNode {
  const found = teams.find((one) => one.view.org === orgId && one.view.team === place.id)
  if (found === undefined) return { kind: 'team', team: place, view: undefined, agents: [], working: 0, waiting: 0, signals: undefined }
  const { view, waitingOnViewer } = found
  const agents = view.people
    .filter((person) => person.kind === 'agent')
    .map((person) => ({ person, presence: presenceOf(person, waitingOnViewer), isLead: person.role === 'lead' }))
  return {
    kind: 'team',
    team: place,
    view,
    agents,
    working: agents.filter((agent) => agent.person.state === 'working').length,
    waiting: agents.filter((agent) => agent.presence === 'needs-you').length,
    signals: summarize(found),
  }
}

const waitingOf = (children: readonly UnitChild[]): number =>
  children.reduce((sum, child) => sum + (child.kind === 'person' || child.kind === 'org' ? 0 : child.waiting), 0)

const mineOf = (children: readonly UnitChild[]): number =>
  children.reduce((sum, child) => sum + (child.kind === 'team' ? (child.signals?.mine ?? 0) : child.kind === 'person' ? 0 : child.mine), 0)

/** A department's departments, teams, and people, in that order. `null` is the top of the organization. */
function childrenOf(parent: string | null, definition: OrgDefinition, teams: readonly Team[], viewerAccount: string): UnitChild[] {
  const departments = definition.departments
    .filter((department) => department.parent === parent)
    .map((department): DepartmentNode => {
      const children = childrenOf(department.id, definition, teams, viewerAccount)
      return { kind: 'department', department, head: definition.people.find((person) => person.id === department.head), children, waiting: waitingOf(children), mine: mineOf(children) }
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

export interface OrgInput {
  org: OrgRef
  /** The org's signed definition, once read. */
  definition: OrgDefinition | undefined
  /** Whether reading it failed. */
  failed: boolean
}

/** Every org of the account side by side, each with its departments, teams, and people, and the signals of the overview. */
export function buildAllOrgsTree(orgs: readonly OrgInput[], teams: readonly Team[], viewerAccount: string): OrgNode[] {
  return orgs.map(({ org, definition, failed }): OrgNode => {
    if (definition === undefined) return { kind: 'org', org, children: [], mine: 0, state: failed ? 'failed' : 'loading' }
    const children = childrenOf(null, definition, teams, viewerAccount)
    return { kind: 'org', org, children, mine: mineOf(children), state: 'ready' }
  })
}
