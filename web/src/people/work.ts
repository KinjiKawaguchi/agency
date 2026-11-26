import type { EpicForecast, WorkItem } from '../api/teamview'

export type SegmentKind = 'done' | 'doing' | 'blocked' | 'rest'

export interface Segment {
  kind: SegmentKind
  count: number
}

export const blockedTotal = (epic: EpicForecast): number => epic.blocked.reduce((sum, entry) => sum + entry.count, 0)

/** The bar of an epic, left to right. Segments with nothing in them are dropped, which also covers counts that overlap. */
export function epicSegments(epic: EpicForecast): Segment[] {
  const blocked = blockedTotal(epic)
  const rest = epic.total - epic.done - epic.doing - blocked
  const segments: Segment[] = [
    { kind: 'done', count: epic.done },
    { kind: 'doing', count: epic.doing },
    { kind: 'blocked', count: blocked },
    { kind: 'rest', count: rest },
  ]
  return segments.filter((segment) => segment.count > 0)
}

/**
 * An epic as the team's view lists it. A finished epic stays in the list for thirty days after its last child
 * was done (`doneAt`), with `total == done` and nothing left; it carries no forecast. `doneAt` is added here until the
 * generated type has it.
 */
export type Epic = EpicForecast & { doneAt?: string }

/** When the epic was finished, in ms: nothing when the view does not say, or says something that is not a time. */
export function doneTime(epic: Epic): number | undefined {
  const time = epic.doneAt === undefined ? Number.NaN : Date.parse(epic.doneAt)
  return Number.isNaN(time) ? undefined : time
}

/** Whether the epic is finished: every child is done. A finished epic is not work left to do, so it is left out of every total of what remains. */
export const isDone = (epic: EpicForecast): boolean => epic.total > 0 && epic.done === epic.total && epic.doing === 0

/** The epics still open first (work going on first, then the furthest along), then the finished ones, the latest finished first. */
export function sortEpics<T extends Epic>(epics: readonly T[]): T[] {
  const progress = (epic: EpicForecast): number => (epic.total === 0 ? 0 : epic.done / epic.total)
  const finishedAt = (epic: Epic): number => doneTime(epic) ?? 0
  return epics.toSorted((a, b) => Number(isDone(a)) - Number(isDone(b)) || (isDone(a) ? finishedAt(b) - finishedAt(a) : b.doing - a.doing || progress(b) - progress(a)))
}

export const inProgress = (work: readonly WorkItem[]): WorkItem[] => work.filter((item) => item.status === 'inProgress')

/** Work that cannot move, other than what is already being worked on or finished. */
export const blockedWork = (work: readonly WorkItem[]): WorkItem[] =>
  work.filter((item) => item.blocked !== undefined && item.status !== 'inProgress' && item.status !== 'done')
