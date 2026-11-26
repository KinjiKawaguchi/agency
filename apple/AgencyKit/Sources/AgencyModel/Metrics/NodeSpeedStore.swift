import AgencyAPI
import Foundation
import Observation

/// Where the node's own response times are read from: the home node's Client API. `NodeClient` is the real one.
public protocol NodeSpeedSource: Sendable {
    func metrics(days: Int) async throws -> Metrics
}

extension NodeClient: NodeSpeedSource {}

/// What one period's read came to.
public enum NodeSpeedAnswer: Equatable, Sendable {
    case ready(NodeSpeed)
    /// The node does not measure (404).
    case unavailable
    /// The node answered, but not in a way that can be read, or could not (503).
    case couldNotAnswer(String)
    case failed(String)
}

/// The node's own response times on the evaluation screen, for the owner, over a period the person chooses.
///
/// The section is the owner's: a node that says it is not (403) has nothing drawn, no alert and no hint, for good. Before the node
/// has answered a period, nothing of it is drawn but a skeleton; a failed refresh leaves what was shown for that period.
@MainActor
@Observable
public final class NodeSpeedStore {
    /// The periods to choose from, in days.
    public static let periods = [1, 7, 30]

    public private(set) var days = 1
    /// What each period's last read came to; a period never read has none, which is not the same as nothing measured.
    public private(set) var answers: [Int: NodeSpeedAnswer] = [:]
    /// The node said the viewer is not its owner: nothing of the section is drawn.
    public private(set) var isHidden = false

    @ObservationIgnored private let source: any NodeSpeedSource

    public init(source: any NodeSpeedSource) {
        self.source = source
    }

    /// What the chosen period came to, or `nil` before it has been read.
    public var answer: NodeSpeedAnswer? { answers[days] }

    public func choose(days: Int) async {
        guard Self.periods.contains(days), days != self.days else { return }
        self.days = days
        await refresh()
    }

    public func refresh() async {
        guard !isHidden else { return }
        let days = days
        let outcome: NodeSpeedAnswer
        do {
            outcome = .ready(NodeSpeed(try await source.metrics(days: days)))
        } catch NodeError.http(403, _) {
            isHidden = true
            return
        } catch NodeError.http(404, _) {
            outcome = .unavailable
        } catch NodeError.http(503, let message) {
            outcome = .couldNotAnswer(message ?? "HTTP 503")
        } catch NodeError.unreadable {
            outcome = .couldNotAnswer(Ja.Evaluation.unreadable)
        } catch where DeskStore.isCancelled(error) {
            return
        } catch {
            outcome = .failed(DeskStore.reason(error))
        }
        // What is on screen for the period stays when a refresh fails.
        if case .ready? = answers[days], !outcome.isReady { return }
        answers[days] = outcome
    }
}

extension NodeSpeedAnswer {
    var isReady: Bool {
        if case .ready = self { true } else { false }
    }
}
