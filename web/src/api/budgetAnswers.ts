// The weekly budget answer comes from the node, which has computed the state, so it is read once, here, and the screens never compare a
// spend with a cap: the state is the node's word. A spend that is missing or not a number refuses the whole answer (a bare 0 would say
// the team spent nothing), and so does a state that is not one of the three.
import type { BudgetPart, BudgetState, BudgetStatus, OrgBudget } from './clientApi'
import { MalformedAnswer } from './evaluationAnswers'

const STATES: ReadonlySet<string> = new Set<BudgetState>(['ok', 'warning', 'over'])

type Raw = Record<string, unknown>
const asObject = (value: unknown): Raw => (typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Raw) : {})

function partOf(raw: unknown, what: string): BudgetPart & OrgBudget {
  const given = asObject(raw)
  const { spentUsd, capUsd, state, partial } = given
  if (typeof spentUsd !== 'number' || !Number.isFinite(spentUsd) || spentUsd < 0) throw new MalformedAnswer(`${what} spend`)
  if (capUsd !== undefined && (typeof capUsd !== 'number' || !Number.isInteger(capUsd) || capUsd < 0)) throw new MalformedAnswer(`${what} cap`)
  if (typeof state !== 'string' || !STATES.has(state)) throw new MalformedAnswer(`${what} state`)
  return { spentUsd, state: state as BudgetState, ...(capUsd === undefined ? {} : { capUsd }), ...(partial === true ? { partial: true } : {}) }
}

export function readBudget(raw: unknown): BudgetStatus {
  const given = asObject(raw)
  if (typeof given.weekStart !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(given.weekStart)) throw new MalformedAnswer('week')
  return { weekStart: given.weekStart, team: partOf(given.team, 'team'), org: partOf(given.org, 'org') }
}
