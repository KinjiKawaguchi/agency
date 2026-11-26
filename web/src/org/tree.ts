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
  /** Absent while the team is being read, and when it could not be read. */
  view: TeamView | undefined
  /** The team is being read (and tried again if it is not ready): its box shows a skeleton, not a failure. */
  isLoading: boolean
  agents: AgentNode[]
  working: number
  /** Where the team's data comes from: a copy of an old read says so on the box. */
  source: Team['source'] | undefined
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
}

/** The `org/team` keys of the teams that are still being read. */
export type Loading = ReadonlySet<string>

function teamNode(place: TeamPlace, teams: readonly Team[], orgId: string, loading: Loading): TeamNode {
  const found = teams.find((one) => one.view.org === orgId && one.view.team === place.id)
  if (found === undefined) return { kind: 'team', team: place, view: undefined, isLoading: loading.has(`${orgId}/${place.id}`), source: undefined, agents: [], working: 0, signals: undefined }
  const { view, waitingOnViewer } = found
  const agents = view.people
    .filter((person) => person.kind === 'agent')
    .map((person) => ({ person, presence: presenceOf(person, waitingOnViewer), isLead: person.role === 'lead' }))
  return {
    kind: 'team',
    team: place,
    view,
    isLoading: false,
    agents,
    working: agents.filter((agent) => agent.person.state === 'working').length,
    source: found.source,
    signals: summarize(found),
  }
}

const mineOf = (children: readonly UnitChild[]): number =>
  children.reduce((sum, child) => sum + (child.kind === 'team' ? (child.signals?.mine ?? 0) : child.kind === 'person' ? 0 : child.mine), 0)

/** A department's departments, teams, and people, in that order. `null` is the top of the organization. */
function childrenOf(parent: string | null, definition: OrgDefinition, teams: readonly Team[], viewerAccount: string, loading: Loading): UnitChild[] {
  const departments = definition.departments
    .filter((department) => department.parent === parent)
    .map((department): DepartmentNode => {
      const children = childrenOf(department.id, definition, teams, viewerAccount, loading)
      return { kind: 'department', department, head: definition.people.find((person) => person.id === department.head), children, mine: mineOf(children) }
    })
  const places = definition.teams.filter((place) => place.department === parent).map((place) => teamNode(place, teams, definition.org, loading))
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
export function buildOrgTree(definition: OrgDefinition, teams: readonly Team[], viewerAccount: string, focusId?: string, loading: Loading = new Set()): OrgTree {
  const whole = childrenOf(null, definition, teams, viewerAccount, loading)
  const focus = focusId === undefined ? undefined : findDepartment(whole, focusId)
  const children = focus === undefined ? whole : focus.children
  return { focus, children }
}

export interface OrgInput {
  org: OrgRef
  /** The org's signed definition, once read. */
  definition: OrgDefinition | undefined
  /** Whether reading it failed. */
  failed: boolean
}

/** Every org of the account side by side, each with its departments, teams, and people, and the signals of the overview. */
export function buildAllOrgsTree(orgs: readonly OrgInput[], teams: readonly Team[], viewerAccount: string, loading: Loading = new Set()): OrgNode[] {
  return orgs.map(({ org, definition, failed }): OrgNode => {
    if (definition === undefined) return { kind: 'org', org, children: [], mine: 0, state: failed ? 'failed' : 'loading' }
    const children = childrenOf(null, definition, teams, viewerAccount, loading)
    return { kind: 'org', org, children, mine: mineOf(children), state: 'ready' }
  })
}
