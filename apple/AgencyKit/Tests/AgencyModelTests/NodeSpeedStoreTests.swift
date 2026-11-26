@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class ScriptedMetrics: NodeSpeedSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _results: [Int: Result<Metrics, Error>] = [:]
    private var _asked: [Int] = []
    var asked: [Int] { lock.withLock { _asked } }
    func set(_ days: Int, _ result: Result<Metrics, Error>) { lock.withLock { _results[days] = result } }
    func metrics(days: Int) async throws -> Metrics {
        try lock.withLock { _asked.append(days); return try (_results[days] ?? .failure(NodeError.http(status: 500, message: nil))).get() }
    }
}

@MainActor
@Suite("Node speed store")
struct NodeSpeedStoreTests {
    private func answer(count: Int) -> Metrics {
        func instant(_ text: String) -> Instant { try! JSONDecoder().decode(Instant.self, from: Data("\"\(text)\"".utf8)) }
        var buckets = Array(repeating: 0, count: LatencySeries.bucketCount)
        buckets[2] = count
        let one = LatencySeries(kind: .serve, name: "GET /api/me", group: "desk", isInNodeRead: true, count: count, buckets: buckets)
        return Metrics(range: (instant("2026-11-24T10:00:00Z"), instant("2026-11-24T12:00:00Z")), generatedAt: instant("2026-11-24T11:30:00Z"), budget: Budget(inNodeReadP95Ms: 100), objectives: [], series: [one])
    }

    private func store(_ source: ScriptedMetrics) -> NodeSpeedStore { NodeSpeedStore(source: source) }

    @Test("nothing is known before the node has answered, and a period is read once it is chosen")
    func firstRead() async {
        let source = ScriptedMetrics()
        source.set(1, .success(answer(count: 4)))
        source.set(7, .success(answer(count: 9)))
        let store = store(source)
        #expect(store.answer == nil && store.answers.isEmpty && !store.isHidden && store.days == 1)
        await store.refresh()
        #expect(store.answer != nil)
        await store.choose(days: 7)
        #expect(store.days == 7 && source.asked == [1, 7])
        guard case .ready(let speed)? = store.answer else { Issue.record("should be ready"); return }
        #expect(speed.routeGroups.first?.row.count == 9, "the period shown is the period read")
    }

    @Test("a period nobody offers is not chosen, and the one already shown is not read again by choosing it")
    func choosing() async {
        let source = ScriptedMetrics()
        source.set(1, .success(answer(count: 4)))
        let store = store(source)
        await store.choose(days: 14)
        await store.choose(days: 1)
        #expect(store.days == 1 && source.asked.isEmpty)
    }

    @Test("a period not read yet has no answer, even when another has one: 7 days are not shown as 1")
    func perPeriod() async {
        let source = ScriptedMetrics()
        source.set(1, .success(answer(count: 4)))
        source.set(7, .failure(NodeError.http(status: 502, message: "down")))
        let store = store(source)
        await store.refresh()
        await store.choose(days: 7)
        #expect(store.answer == .failed("down"), "the period asked has its own answer, and 1 day's is not shown for it")
        await store.choose(days: 1)
        #expect(store.answer?.isReady == true)
    }

    @Test("the node has answered once any period has an answer, whatever it came to, and a 403 is no answer")
    func hasAnswered() async {
        let source = ScriptedMetrics()
        let store = store(source)
        #expect(!store.hasAnswered)
        source.set(1, .failure(NodeError.http(status: 500, message: "boom")))
        await store.refresh()
        #expect(store.hasAnswered && store.answer == .failed("boom"))
        let notOwner = ScriptedMetrics()
        notOwner.set(1, .failure(NodeError.http(status: 403, message: nil)))
        let other = self.store(notOwner)
        await other.refresh()
        #expect(!other.hasAnswered && other.isHidden, "nothing was drawn before it, and nothing is left behind")
    }

    @Test("403 hides the section for good, and nothing more is asked")
    func notTheOwner() async {
        let source = ScriptedMetrics()
        source.set(1, .failure(NodeError.http(status: 403, message: "owner only")))
        let store = store(source)
        await store.refresh()
        #expect(store.isHidden && store.answer == nil)
        await store.refresh()
        await store.choose(days: 7)
        #expect(source.asked == [1])
    }

    @Test("404 says the node does not measure; 503 and a reading that is refused say it could not answer; the rest is a failure")
    func failures() async {
        for (error, expected) in [
            (NodeError.http(status: 404, message: nil), NodeSpeedAnswer.unavailable),
            (NodeError.http(status: 503, message: "busy"), .couldNotAnswer),
            (NodeError.http(status: 503, message: nil), .couldNotAnswer),
            (NodeError.unreadable("buckets"), .couldNotAnswer),
            (NodeError.http(status: 500, message: "boom"), .failed("boom")),
        ] {
            let source = ScriptedMetrics()
            source.set(1, .failure(error))
            let store = store(source)
            await store.refresh()
            #expect(store.answer == expected, Comment(rawValue: "\(error)"))
        }
    }

    @Test("what is shown stays when a refresh fails, and goes when the node answers again")
    func staysWhenFailing() async {
        let source = ScriptedMetrics()
        source.set(1, .success(answer(count: 4)))
        let store = store(source)
        await store.refresh()
        source.set(1, .failure(NodeError.http(status: 502, message: "down")))
        await store.refresh()
        #expect(store.answer?.isReady == true)
        source.set(1, .failure(NodeError.http(status: 404, message: nil)))
        await store.refresh()
        #expect(store.answer?.isReady == true, "a node that says it does not measure after it did is no answer worth showing over numbers")
        source.set(1, .success(answer(count: 8)))
        await store.refresh()
        guard case .ready(let speed)? = store.answer else { Issue.record("should be ready"); return }
        #expect(speed.routeGroups.first?.row.count == 8)
    }

    @Test("a period whose read was cancelled has no answer, while the node has answered another: the heading stays and the numbers are not the other period's")
    func otherPeriodUnread() async {
        let source = ScriptedMetrics()
        source.set(1, .success(answer(count: 4)))
        source.set(7, .failure(CancellationError()))
        let store = store(source)
        await store.refresh()
        await store.choose(days: 7)
        #expect(store.answer == nil && store.hasAnswered)
    }

    @Test("a read that is cancelled says nothing")
    func cancelled() async {
        let source = ScriptedMetrics()
        source.set(1, .failure(CancellationError()))
        let store = store(source)
        await store.refresh()
        #expect(store.answer == nil)
    }
}
