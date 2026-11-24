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

/** Epics with work going on first, then the ones furthest along. */
export function sortEpics(epics: readonly EpicForecast[]): EpicForecast[] {
  const progress = (epic: EpicForecast): number => (epic.total === 0 ? 0 : epic.done / epic.total)
  return epics.toSorted((a, b) => b.doing - a.doing || progress(b) - progress(a))
}

export const inProgress = (work: readonly WorkItem[]): WorkItem[] => work.filter((item) => item.status === 'inProgress')

/** Work that cannot move, other than what is already being worked on or finished. */
export const blockedWork = (work: readonly WorkItem[]): WorkItem[] =>
  work.filter((item) => item.blocked !== undefined && item.status !== 'inProgress' && item.status !== 'done')
