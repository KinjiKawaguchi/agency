import AgencyAPI
import Foundation

/// What the screen draws of a team's work and epics, ported from `web/src/people/work.ts`.
public enum Work {
    public enum SegmentKind: Sendable { case done, doing, blocked, rest }

    public struct Segment: Equatable, Sendable {
        public let kind: SegmentKind
        public let count: Int
    }

    public static func blockedTotal(_ epic: EpicForecast) -> Int {
        epic.blocked.reduce(0) { $0 + $1.count }
    }

    /// The bar of an epic, left to right. Segments with nothing in them are dropped, which also covers counts that overlap.
    public static func segments(of epic: EpicForecast) -> [Segment] {
        let blocked = blockedTotal(epic)
        return [
            Segment(kind: .done, count: epic.done),
            Segment(kind: .doing, count: epic.doing),
            Segment(kind: .blocked, count: blocked),
            Segment(kind: .rest, count: epic.total - epic.done - epic.doing - blocked),
        ].filter { $0.count > 0 }
    }

    /// Epics with work going on first, then the ones furthest along, and the finished ones after every open epic;
    /// the team's own order breaks a tie.
    public static func sorted(_ epics: [EpicForecast]) -> [EpicForecast] {
        func progress(_ epic: EpicForecast) -> Double { epic.total == 0 ? 0 : Double(epic.done) / Double(epic.total) }
        return epics.enumerated().sorted { left, right in
            if left.element.isFinished != right.element.isFinished { return !left.element.isFinished }
            if left.element.doing != right.element.doing { return left.element.doing > right.element.doing }
            if progress(left.element) != progress(right.element) { return progress(left.element) > progress(right.element) }
            return left.offset < right.offset
        }.map(\.element)
    }

    public static func inProgress(_ work: [WorkItem]) -> [WorkItem] {
        work.filter { $0.status == .inProgress }
    }

    /// Work that cannot move, other than what is already being worked on or finished.
    public static func blocked(_ work: [WorkItem]) -> [WorkItem] {
        work.filter { $0.blocked != nil && $0.status != .inProgress && $0.status != .done }
    }
}
