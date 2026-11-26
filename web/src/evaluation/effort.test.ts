import type { DayEffort, EffortType, TeamEffort } from '../api/evaluationApi'
import { EFFORT_TYPES, planOf, pointsFromPlan, sharesOf, totalCost, weekSpend } from './effort'

const zero = (): Record<EffortType, number> => ({ newFeature: 0, improvement: 0, maintenance: 0, debt: 0, incident: 0, unclassified: 0 })
const tokens = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }
const effortOf = (costs: Partial<Record<EffortType, number>>): TeamEffort => ({
  team: 'app',
  byType: Object.fromEntries(EFFORT_TYPES.map((type) => [type, { costUsd: costs[type] ?? 0, tokens }])) as TeamEffort['byType'],
  byDay: [],
})

const allocation = { newFeature: 50, improvement: 20, maintenance: 20, debt: 10, incident: 0 }
const definitionWith = (plan: unknown) => ({ teams: [{ id: 'other' }, { id: 'app', plan }] })

describe('planOf', () => {
  it('reads a plan that has the shape the protocol allows', () => {
    expect(planOf(definitionWith({ allocation, weeklyBudgetUsd: 140 }), 'app')).toEqual({ allocation, weeklyBudgetUsd: 140 })
  })

  it('says there is none for a team that has none, an unknown team, or a definition that is not one', () => {
    expect(planOf(definitionWith(undefined), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation, weeklyBudgetUsd: 1 }), 'nobody')).toBeUndefined()
    expect(planOf(null, 'app')).toBeUndefined()
    expect(planOf({ teams: 'x' }, 'app')).toBeUndefined()
  })

  it('refuses a plan whose shares do not sum to 100', () => {
    expect(planOf(definitionWith({ allocation: { ...allocation, debt: 11 }, weeklyBudgetUsd: 140 }), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation: { ...allocation, debt: 9 }, weeklyBudgetUsd: 140 }), 'app')).toBeUndefined()
  })

  it('refuses shares and budgets that are not whole, are negative, or are missing', () => {
    expect(planOf(definitionWith({ allocation: { ...allocation, newFeature: 49.5, debt: 10.5 }, weeklyBudgetUsd: 140 }), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation: { ...allocation, newFeature: 110, debt: -10 }, weeklyBudgetUsd: 140 }), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation, weeklyBudgetUsd: 12.5 }), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation, weeklyBudgetUsd: -1 }), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation }), 'app')).toBeUndefined()
    expect(planOf(definitionWith({ allocation: { newFeature: 100 }, weeklyBudgetUsd: 1 }), 'app')).toBeUndefined()
  })
})

describe('shares', () => {
  it('divides each type’s cost by the whole, the unclassified work included', () => {
    const shares = sharesOf(effortOf({ newFeature: 30, maintenance: 50, unclassified: 20 }))
    expect(shares).toMatchObject({ newFeature: 30, improvement: 0, maintenance: 50, unclassified: 20 })
    expect(totalCost(effortOf({ newFeature: 30, unclassified: 20 }))).toBe(50)
  })

  it('is all zero, not not-a-number, when nothing was spent', () => {
    expect(Object.values(sharesOf(effortOf({})))).toEqual([0, 0, 0, 0, 0, 0])
  })

  it('measures the gap to the plan in whole points', () => {
    expect(pointsFromPlan(43.4, 50)).toBe(-7)
    expect(pointsFromPlan(20.4, 20)).toBe(0)
  })
})

describe('weekSpend', () => {
  const day = (date: string, newFeature: number, unclassified = 0): DayEffort => ({ date, costUsd: { ...zero(), newFeature, unclassified } })

  it('adds up the seven days that end at the last day of the period, and no earlier day', () => {
    const byDay = [day('2026-11-19', 100), day('2026-11-20', 1), day('2026-11-24', 2, 1), day('2026-11-26', 3)]
    expect(weekSpend(byDay, '2026-11-26')).toBe(7)
  })
})
