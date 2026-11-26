import type { OrgDefinition } from '../api/clientApi'
import { sampleTeamView } from '../api/teamview'
import { ORG_DEFINITION, siteTeamView } from '../mock/fixtures'
import { teamOf } from '../test/views'
import { buildAllOrgsTree, buildOrgTree, type DepartmentNode, type OrgNode, type TeamNode, type UnitChild } from './tree'

const views = [teamOf(sampleTeamView), teamOf(siteTeamView(sampleTeamView))]

const department = (children: readonly UnitChild[], id: string): DepartmentNode | undefined =>
  children.find((child): child is DepartmentNode => child.kind === 'department' && child.department.id === id)

describe('buildOrgTree', () => {
  const tree = buildOrgTree(ORG_DEFINITION, views, 'alice')

  it('nests departments, teams, and people under their department', () => {
    expect(tree.children.map((child) => child.kind)).toEqual(['department', 'department'])
    const engineering = department(tree.children, 'engineering')
    expect(engineering?.children.map((child) => child.kind)).toEqual(['department', 'team', 'person'])
    expect(department(engineering?.children ?? [], 'platform')?.children.map((child) => child.kind)).toEqual(['person'])
  })

  it('names the head of a department', () => {
    expect(department(tree.children, 'engineering')?.head?.label).toBe('Alice')
    expect(department(tree.children, 'design')?.head?.label).toBe('Carol')
  })

  it('marks the viewer by account, not by name', () => {
    const engineering = department(tree.children, 'engineering')
    const people = engineering?.children.filter((child) => child.kind === 'person') ?? []
    expect(people.map((child) => child.kind === 'person' && child.isYou)).toEqual([true])
  })

  it('lists the agents of a team and counts who waits for the viewer, rolled up the tree', () => {
    const team = department(tree.children, 'engineering')?.children.find((child): child is TeamNode => child.kind === 'team')
    expect(team?.agents.map((agent) => agent.person.id)).toEqual(['lead', 'backend', 'frontend', 'docs'])
    expect(team?.agents.find((agent) => agent.isLead)?.person.id).toBe('lead')
    expect(team).toMatchObject({ working: 2, waiting: 1 })
    expect(department(tree.children, 'engineering')?.waiting).toBe(1)
    expect(department(tree.children, 'design')?.waiting).toBe(0)
    expect(tree.waiting).toBe(1)
  })

  it('keeps a team whose view could not be read, without agents', () => {
    const partial = buildOrgTree(ORG_DEFINITION, [teamOf(sampleTeamView)], 'alice')
    const site = department(partial.children, 'design')?.children.find((child): child is TeamNode => child.kind === 'team')
    expect(site).toMatchObject({ view: undefined, agents: [] })
  })

  it('focuses on one department and what is below it', () => {
    const focused = buildOrgTree(ORG_DEFINITION, views, 'alice', 'engineering')
    expect(focused.focus?.department.id).toBe('engineering')
    expect(focused.children.map((child) => child.kind)).toEqual(['department', 'team', 'person'])
  })

  it('shows the whole organization when the focus is unknown', () => {
    expect(buildOrgTree(ORG_DEFINITION, views, 'alice', 'nowhere').focus).toBeUndefined()
  })

  it('leaves out departments that cannot be reached from the top, such as a loop of parents', () => {
    const looping: OrgDefinition = {
      ...ORG_DEFINITION,
      departments: [
        { id: 'a', label: 'A', parent: 'b', head: null },
        { id: 'b', label: 'B', parent: 'a', head: null },
      ],
      people: [],
      teams: [],
    }
    expect(buildOrgTree(looping, [], 'alice').children).toEqual([])
  })
})

describe('buildAllOrgsTree', () => {
  const acme = { id: 'acme', label: 'Acme', isRoot: true, teams: [] }
  const lab = { id: 'lab', label: 'Lab', isRoot: true, teams: [] }
  const club = { id: 'club', label: 'Club', isRoot: true, teams: [] }
  const labDefinition: OrgDefinition = { ...ORG_DEFINITION, org: 'lab', label: 'Lab', departments: [{ id: 'main', label: 'Main', parent: null, head: 'owner' }], teams: [{ id: 'app', label: 'App', department: 'main', node: 'https://alice.example' }] }
  const team = (node: OrgNode): TeamNode | undefined => {
    const main = node.children.find((child): child is DepartmentNode => child.kind === 'department')
    return main?.children.find((child): child is TeamNode => child.kind === 'team')
  }

  it('puts every org side by side, in the order of the account, each with its own tree', () => {
    const orgs = buildAllOrgsTree([{ org: acme, definition: ORG_DEFINITION, failed: false }, { org: lab, definition: labDefinition, failed: false }], views, 'alice')
    expect(orgs.map((node) => [node.org.id, node.state])).toEqual([['acme', 'ready'], ['lab', 'ready']])
    expect(orgs[0]?.children.map((child) => child.kind)).toEqual(['department', 'department'])
  })

  it('carries the overview’s signals on each team, matched to its own org', () => {
    // The `app` team exists in both orgs; only the one of `acme` has a view here.
    const [first, second] = buildAllOrgsTree([{ org: acme, definition: ORG_DEFINITION, failed: false }, { org: lab, definition: labDefinition, failed: false }], views, 'alice')
    const engineering = first?.children.find((child): child is DepartmentNode => child.kind === 'department' && child.department.id === 'engineering')
    const app = engineering?.children.find((child): child is TeamNode => child.kind === 'team')
    expect(app?.signals).toMatchObject({ attention: 'mine', mine: views[0]?.mine.size })
    expect(views[0]?.mine.size).toBeGreaterThan(0)
    expect(app?.signals?.participants).toBeGreaterThan(0)
    expect(second === undefined ? undefined : team(second)?.signals).toBeUndefined()
  })

  it('adds up the requests that are the viewer’s, by department and by org, from the node’s list', () => {
    const [first] = buildAllOrgsTree([{ org: acme, definition: ORG_DEFINITION, failed: false }], views, 'alice')
    const engineering = first?.children.find((child): child is DepartmentNode => child.kind === 'department' && child.department.id === 'engineering')
    const listed = views[0]?.mine.size
    expect(listed).toBeGreaterThan(0)
    expect(engineering?.mine).toBe(listed)
    expect(first?.mine).toBe(listed)
    const design = first?.children.find((child): child is DepartmentNode => child.kind === 'department' && child.department.id === 'design')
    expect(design?.mine).toBe(0)
  })

  it('counts nothing for a team whose view the viewer has none of, since the node lists no request of it', () => {
    const [first] = buildAllOrgsTree([{ org: acme, definition: ORG_DEFINITION, failed: false }], [], 'alice')
    expect(first?.mine).toBe(0)
  })

  it('keeps an org whose definition is still loading or could not be read, and draws the rest', () => {
    const orgs = buildAllOrgsTree([{ org: acme, definition: ORG_DEFINITION, failed: false }, { org: lab, definition: undefined, failed: false }, { org: club, definition: undefined, failed: true }], views, 'alice')
    expect(orgs.map((node) => node.state)).toEqual(['ready', 'loading', 'failed'])
    expect(orgs[1]?.children).toEqual([])
    expect(orgs[2]?.children).toEqual([])
  })
})
