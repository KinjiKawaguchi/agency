import type { OrgDefinition } from '../api/clientApi'
import { sampleTeamView } from '../api/teamview'
import { ORG_DEFINITION, siteTeamView } from '../mock/fixtures'
import { teamOf } from '../test/views'
import { buildOrgTree, type DepartmentNode, type TeamNode, type UnitChild } from './tree'

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
        { id: 'a', label: 'A', parent: 'b' },
        { id: 'b', label: 'B', parent: 'a' },
      ],
      people: [],
      teams: [],
    }
    expect(buildOrgTree(looping, [], 'alice').children).toEqual([])
  })
})
