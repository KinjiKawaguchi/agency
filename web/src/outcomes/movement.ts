import type { KpiInfo } from '../api/outcomesApi'

export type Direction = 'rose' | 'fell' | 'unchanged'
/** Better or worse by the KPI's `goodWhen`; nothing when the node did not say, or said something this client does not know: it is not coloured. */
export type Quality = 'better' | 'worse' | undefined

export interface Movement {
  direction: Direction
  quality: Quality
}

/** How a number moved from a baseline to a later one. It says nothing of why. */
export function movementOf(from: number, to: number, goodWhen: KpiInfo['goodWhen']): Movement {
  const direction: Direction = to > from ? 'rose' : to < from ? 'fell' : 'unchanged'
  if (direction === 'unchanged' || goodWhen === undefined) return { direction, quality: undefined }
  const isGood = (direction === 'rose') === (goodWhen === 'higher')
  return { direction, quality: isGood ? 'better' : 'worse' }
}
