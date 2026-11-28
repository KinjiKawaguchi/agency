import AgencyAPI
import Foundation

/// Where a report goes: the home node's `POST /api/telemetry`.
public protocol TelemetrySending: Sendable {
    func reportTelemetry(_ report: TelemetryReport) async throws -> TelemetryAnswer
}

extension NodeClient: TelemetrySending {}

/// What the app observed of its calls to its home node, kept in memory and sent to the node, which shows it beside its own timing as the
/// `client` series (`docs/protocol.md`, "Metrics"): so that a slow network or a slow app can be told from a slow node.
///
/// It never waits for anything a call does: `record` only puts a sample in a list, and a report goes out from a task of its own, at most once a
/// minute and when the app leaves the foreground. What the node refuses is dropped and not sent again; a refusal that says the node will not
/// take reports from this app (not the owner, no such route, too large) ends reporting for the session. Nothing is kept between sessions.
@MainActor
public final class ClientTelemetry {
    /// The shortest time between two reports.
    public static let interval: TimeInterval = 60
    /// What waits at most: older samples go first, so that a node that cannot be told does not make the app hold them all.
    static let maxWaiting = 1_000

    private let sender: any TelemetrySending
    private let standing: @MainActor () -> Standing
    private let now: @Sendable () -> Date
    private var waiting: [CallSample] = []
    private var lastSent: Date
    private var isSending = false
    /// The send that a sample started, so that a test can wait for it.
    private var started: Task<Void, Never>?
    /// The node said it will not take reports from this app: nothing more is recorded or sent in this session.
    public private(set) var isStopped = false

    /// - Parameter standing: what the node said the viewer is. Only the owner's reports are taken, so a guest sends none and nothing of a
    ///   guest's calls is kept; until the node has said, what is seen waits, and the calls that come first (the account, the session) are the
    ///   ones worth reporting. It is asked when a sample comes and when a report would go.
    public init(sender: any TelemetrySending, standing: @escaping @MainActor () -> Standing, now: @escaping @Sendable () -> Date = Date.init) {
        self.sender = sender
        self.standing = standing
        self.now = now
        lastSent = now()
    }

    /// Puts what one call came to in the list. A report goes out when the last one is a minute old.
    public func record(_ sample: CallSample) {
        guard !isStopped, !standing().isGuest else { return }
        waiting.append(sample)
        if waiting.count > Self.maxWaiting { waiting.removeFirst(waiting.count - Self.maxWaiting) }
        if now().timeIntervalSince(lastSent) >= Self.interval {
            started = Task { await send() }
        }
    }

    /// Waits for the send that the last sample started.
    func settled() async {
        await started?.value
    }

    /// The app leaves the foreground: what waits goes now, whatever the minute says.
    public func leftForeground() async {
        await send()
    }

    private func send() async {
        guard !isStopped, !isSending else { return }
        switch standing() {
        case .member: break
        case .guest:
            waiting.removeAll()
            return
        case .loading, .unknown: return
        }
        let report = TelemetryReport.fitting(waiting)
        guard !report.samples.isEmpty else { return }
        waiting.removeFirst(report.samples.count)
        isSending = true
        lastSent = now()
        defer { isSending = false }
        do {
            _ = try await sender.reportTelemetry(report)
        } catch NodeError.http(let status, _) where Self.refusesReports(status) {
            isStopped = true
            waiting.removeAll()
        } catch {
            // What did not get to the node is not sent again: a report is a sample of what was seen, not a record to keep.
        }
    }

    /// A refusal that another try will not change: not the owner (403), no such route (404, 405), a report too large (413) or of a kind the node
    /// does not read (415). Any other refusal, and a call that got no answer, drops that report only.
    static func refusesReports(_ status: Int) -> Bool {
        [400, 403, 404, 405, 413, 415].contains(status)
    }
}
