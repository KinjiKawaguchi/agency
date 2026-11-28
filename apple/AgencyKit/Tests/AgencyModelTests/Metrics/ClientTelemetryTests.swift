import AgencyAPI
import AgencyKit
import Foundation
import Testing
@testable import AgencyModel

/// Takes the reports and answers as told; remembers what it was given.
private actor Sender: TelemetrySending {
    private(set) var reports: [TelemetryReport] = []
    var failure: (any Error)?
    private var gates: [CheckedContinuation<Void, Never>] = []
    var isHolding = false

    func fail(with error: (any Error)?) { failure = error }
    func hold() { isHolding = true }
    func release() {
        isHolding = false
        let waiting = gates
        gates = []
        for gate in waiting { gate.resume() }
    }

    func reportTelemetry(_ report: TelemetryReport) async throws -> TelemetryAnswer {
        reports.append(report)
        if isHolding { await withCheckedContinuation { gates.append($0) } }
        if let failure { throw failure }
        return TelemetryAnswer(accepted: report.samples.count, dropped: 0)
    }
}

/// A clock and a standing that a test moves.
@MainActor
private final class Scene {
    final class Clock: @unchecked Sendable {
        private let lock = NSLock()
        private var value = Date(timeIntervalSince1970: 1_795_000_000)
        var now: Date { lock.withLock { value } }
        func advance(_ seconds: TimeInterval) { lock.withLock { value = value.addingTimeInterval(seconds) } }
    }

    let sender = Sender()
    let clock = Clock()
    var standing: Standing = .member
    private(set) var telemetry: ClientTelemetry!

    init() {
        let clock = clock
        telemetry = ClientTelemetry(sender: sender, standing: { [unowned self] in standing }, now: { clock.now })
    }

    func sample(_ ms: Int = 5) -> CallSample { CallSample(route: "GET /api/me", status: "2xx", ms: ms) }
}

@MainActor
@Suite("Reporting what the app observed")
struct ClientTelemetryTests {
    @Test("nothing goes before a minute has passed; a sample after it sends what waits, in one report")
    func aMinute() async {
        let scene = Scene()
        scene.telemetry.record(scene.sample(1))
        scene.clock.advance(59)
        scene.telemetry.record(scene.sample(2))
        await scene.telemetry.settled()
        #expect(await scene.sender.reports.isEmpty)
        scene.clock.advance(1)
        scene.telemetry.record(scene.sample(3))
        await scene.telemetry.settled()
        #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[1, 2, 3]])
    }

    @Test("two reports are a minute apart at least")
    func notOftener() async {
        let scene = Scene()
        scene.clock.advance(60)
        scene.telemetry.record(scene.sample(1))
        await scene.telemetry.settled()
        scene.clock.advance(30)
        scene.telemetry.record(scene.sample(2))
        await scene.telemetry.settled()
        #expect(await scene.sender.reports.count == 1)
        scene.clock.advance(30)
        scene.telemetry.record(scene.sample(3))
        await scene.telemetry.settled()
        #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[1], [2, 3]])
    }

    @Test("leaving the foreground sends what waits once, whatever the minute says, and nothing when nothing waits")
    func leavingTheForeground() async {
        let scene = Scene()
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.isEmpty)
        scene.telemetry.record(scene.sample(1))
        scene.telemetry.record(scene.sample(2))
        await scene.telemetry.leftForeground()
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[1, 2]])
    }

    @Test("a guest sends none, and what was seen before the node said so is dropped")
    func guest() async {
        let scene = Scene()
        scene.standing = .guest(GuestRef(member: "alice", org: "acme"))
        scene.telemetry.record(scene.sample())
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.isEmpty)

        let later = Scene()
        later.standing = .loading
        later.telemetry.record(later.sample())
        later.standing = .guest(GuestRef(member: "alice", org: "acme"))
        await later.telemetry.leftForeground()
        later.standing = .member
        await later.telemetry.leftForeground()
        #expect(await later.sender.reports.isEmpty)
    }

    @Test("nothing of a guest's calls is kept: when the viewer is a member later, those samples are not sent")
    func guestSamplesAreNotKept() async {
        let scene = Scene()
        scene.standing = .guest(GuestRef(member: "alice", org: "acme"))
        scene.telemetry.record(scene.sample(1))
        scene.standing = .member
        scene.telemetry.record(scene.sample(2))
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[2]], "only what was seen as a member")
    }

    @Test("until the node has said what the viewer is, what was seen waits, and the first calls are in the first report")
    func untilKnown() async {
        let scene = Scene()
        scene.standing = .loading
        scene.telemetry.record(scene.sample(1))
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.isEmpty)
        scene.standing = .unknown
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.isEmpty)
        scene.standing = .member
        scene.telemetry.record(scene.sample(2))
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[1, 2]])
    }

    @Test("a refusal that another try cannot change ends reporting for the session, and what waits is dropped")
    func refusal() async {
        for status in [400, 403, 404, 405, 413, 415] {
            let scene = Scene()
            await scene.sender.fail(with: NodeError.http(status: status, message: "no"))
            scene.telemetry.record(scene.sample(1))
            await scene.telemetry.leftForeground()
            #expect(scene.telemetry.isStopped, Comment(rawValue: "\(status)"))
            scene.telemetry.record(scene.sample(2))
            await scene.telemetry.leftForeground()
            #expect(await scene.sender.reports.count == 1, Comment(rawValue: "\(status)"))
        }
    }

    @Test("any other failure drops that report only: it is not sent again, and the next samples go")
    func otherFailure() async {
        for error: any Error in [NodeError.http(status: 500, message: nil), NodeError.http(status: 429, message: nil), NodeError.notSignedIn, NodeError.sessionExpired, URLError(.timedOut)] {
            let scene = Scene()
            await scene.sender.fail(with: error)
            scene.telemetry.record(scene.sample(1))
            await scene.telemetry.leftForeground()
            await scene.sender.fail(with: nil)
            scene.telemetry.record(scene.sample(2))
            await scene.telemetry.leftForeground()
            #expect(!scene.telemetry.isStopped)
            #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[1], [2]], Comment(rawValue: "\(error)"))
        }
    }

    @Test("a report holds at most 200 samples; the rest goes in the next")
    func atMost200() async {
        let scene = Scene()
        for ms in 0..<450 { scene.telemetry.record(scene.sample(ms)) }
        await scene.telemetry.leftForeground()
        await scene.telemetry.leftForeground()
        await scene.telemetry.leftForeground()
        let reports = await scene.sender.reports
        #expect(reports.map(\.samples.count) == [200, 200, 50])
        #expect(reports.flatMap(\.samples).map(\.ms) == Array(0..<450))
    }

    @Test("a node that cannot be told does not make the app keep every sample: the oldest go first")
    func waitingIsBounded() async {
        let scene = Scene()
        scene.standing = .loading
        for ms in 0..<1_300 { scene.telemetry.record(scene.sample(ms)) }
        scene.standing = .member
        await scene.telemetry.leftForeground()
        let first = await scene.sender.reports.first?.samples.first?.ms
        #expect(first == 300)
    }

    @Test("the phone reports and the Mac does not: the node's series is named for the phone, and a Mac would hide what a phone on a mobile network sees")
    func platforms() {
        #expect(ClientTelemetry.reports(from: .ios))
        #expect(!ClientTelemetry.reports(from: .macos))
    }

    @Test("a report is not sent while another is on its way")
    func oneAtATime() async {
        let scene = Scene()
        await scene.sender.hold()
        scene.telemetry.record(scene.sample(1))
        let first = Task { await scene.telemetry.leftForeground() }
        while await scene.sender.reports.isEmpty { await Task.yield() }
        scene.telemetry.record(scene.sample(2))
        let second = Task { await scene.telemetry.leftForeground() }
        for _ in 0..<50 { await Task.yield() }
        #expect(await scene.sender.reports.count == 1)
        await scene.sender.release()
        await first.value
        await second.value
        await scene.telemetry.leftForeground()
        #expect(await scene.sender.reports.map { $0.samples.map(\.ms) } == [[1], [2]])
    }
}
