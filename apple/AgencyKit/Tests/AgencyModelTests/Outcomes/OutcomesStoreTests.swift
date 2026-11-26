import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeOutcomesSource: OutcomesSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _me: Result<Me, Error> = .success(try! JSONDecoder().decode(Me.self, from: Data(FakeOutcomesSource.twoOrgs.utf8)))
    private var _outcomes: @Sendable (String, Int) throws -> Outcomes = { _, _ in try OutcomeSamples.outcomes() }
    private var _readings: @Sendable (String) throws -> Readings = { _ in try OutcomeSamples.readings() }
    private var gate: CheckedContinuation<Void, Never>?
    private var holdsNext = false
    private(set) var daysAsked: [Int] = []

    static let twoOrgs = """
    {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[
    {"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"App","location":"local","viewer":"owner"}]},
    {"id":"lab","isRoot":false,"label":"Lab","teams":[]}]}
    """

    func set(me: Result<Me, Error>) { lock.withLock { _me = me } }
    func set(outcomes: @escaping @Sendable (String, Int) throws -> Outcomes) { lock.withLock { _outcomes = outcomes } }
    func set(readings: @escaping @Sendable (String) throws -> Readings) { lock.withLock { _readings = readings } }
    func holdNext() { lock.withLock { holdsNext = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    var isHolding: Bool { lock.withLock { gate != nil } }

    func me() async throws -> Me { try lock.withLock { try _me.get() } }

    func outcomes(org: String, days: Int) async throws -> Outcomes {
        if lock.withLock({ let held = holdsNext; holdsNext = false; return held }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        let make = lock.withLock { daysAsked.append(days); return _outcomes }
        return try make(org, days)
    }

    func readings(org: String) async throws -> Readings {
        let make = lock.withLock { _readings }
        return try make(org)
    }
}

@MainActor
@Suite("OutcomesStore")
struct OutcomesStoreTests {
    private let source = FakeOutcomesSource()

    private func makeStore() -> OutcomesStore {
        OutcomesStore(source: source, retry: .never)
    }

    private let forbidden = NodeError.http(status: 403, message: "the numbers are for the owner")

    @Test("the owner's answer gives the page: a card for each KPI and a line for each aim, and the page is offered")
    func owner() async throws {
        let store = makeStore()
        source.set(outcomes: { org, _ in
            if org == "lab" { throw NodeError.http(status: 403, message: nil) }
            return try OutcomeSamples.outcomes()
        })
        await store.refresh()
        #expect(store.loading == .loaded && store.hasAnswered && store.isAvailable)
        #expect(store.orgs.map(\.id) == ["acme"], "the org the owner may not read is not there, and nothing says so")
        let acme = try #require(store.orgs.first)
        #expect(acme.cards.map(\.label) == ["Monthly revenue", "Activation", "Lead time of a feature"])
        guard case .lines(let lines) = acme.readings else { Issue.record("no readings"); return }
        #expect(lines.count == 8)
        #expect(lines[0].team == "App", "the team's own label")
        #expect(lines[0].headline.contains("10月15日") && !lines[0].headline.contains("年"), "short days: the readings are of the year of their day")
    }

    @Test("when every org refuses, there is nothing to show, no failure, and the page is not offered")
    func notOwner() async {
        source.set(outcomes: { _, _ in throw NodeError.http(status: 403, message: nil) })
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.orgs.isEmpty)
        #expect(!store.isAvailable)
        #expect(store.hasAnswered)
    }

    @Test("a node that does not know the org (404) is refused the same way, without a word")
    func notFound() async {
        source.set(outcomes: { _, _ in throw NodeError.http(status: 404, message: nil) })
        let store = makeStore()
        await store.refresh()
        #expect(!store.isAvailable && store.orgs.isEmpty && store.loading == .loaded)
    }

    @Test("before any answer the page is not offered, and a read that fails before one ever worked does not offer it")
    func notOfferedBeforeAnswer() async {
        let store = makeStore()
        #expect(!store.isAvailable && !store.hasAnswered && store.loading == .idle)
        source.set(outcomes: { _, _ in throw NodeError.http(status: 500, message: "broken") })
        await store.refresh()
        #expect(store.loading == .failed("broken"))
        #expect(!store.isAvailable && !store.hasAnswered)
    }

    @Test("once it worked the page stays: a later failure keeps what was read and says it failed")
    func stays() async {
        let store = makeStore()
        await store.refresh()
        #expect(store.isAvailable && store.orgs.count == 2)
        source.set(outcomes: { _, _ in throw NodeError.http(status: 503, message: "down") })
        await store.refresh()
        #expect(store.loading == .failed("down"))
        #expect(store.isAvailable && store.hasAnswered && store.orgs.count == 2)
    }

    @Test("a refresh under way keeps the answer and the offer")
    func underWay() async {
        let store = makeStore()
        await store.refresh()
        source.holdNext()
        let refreshing = Task { await store.refresh() }
        while !source.isHolding { await Task.yield() }
        #expect(store.loading == .loading)
        #expect(store.isAvailable && store.hasAnswered && !store.orgs.isEmpty)
        source.release()
        await refreshing.value
        #expect(store.loading == .loaded)
    }

    @Test("when the owner is later refused everywhere the page is no longer offered")
    func refusedLater() async {
        let store = makeStore()
        await store.refresh()
        source.set(outcomes: { _, _ in throw NodeError.http(status: 403, message: nil) })
        await store.refresh()
        #expect(!store.isAvailable && store.orgs.isEmpty)
    }

    @Test("the readings failing does not take the cards away, and says so")
    func readingsFail() async throws {
        source.set(readings: { _ in throw NodeError.http(status: 500, message: "no readings") })
        let store = makeStore()
        await store.refresh()
        let acme = try #require(store.orgs.first)
        #expect(!acme.cards.isEmpty)
        #expect(acme.readings == .failed("no readings"))
    }

    @Test("a node that does not serve the readings (404) has the cards and no readings")
    func readingsUnavailable() async throws {
        source.set(readings: { _ in throw NodeError.http(status: 404, message: nil) })
        let store = makeStore()
        await store.refresh()
        #expect(try #require(store.orgs.first).readings == .unavailable)
    }

    @Test("the period is 90 days to begin with, may be 30 or 90, and a choice reads again")
    func period() async {
        let store = makeStore()
        #expect(store.days == 90 && OutcomesStore.periods == [30, 90])
        await store.refresh()
        await store.choose(days: 14)
        #expect(store.days == 90, "a period the page does not offer is ignored")
        await store.choose(days: 30)
        #expect(store.days == 30)
        #expect(source.daysAsked.suffix(2) == [30, 30])
        #expect(source.daysAsked.first == 90)
    }

    @Test("an ended sign-in is asked for again, and not shown as a failure of the numbers")
    func signIn() async {
        source.set(me: .failure(NodeError.sessionExpired))
        let store = makeStore()
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(!store.isAvailable)
    }

    @Test("a read that is cancelled after the page answered leaves it as it was: the numbers do not turn back into bars")
    func cancelledKeepsTheAnswer() async {
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        source.set(outcomes: { _, _ in throw CancellationError() })
        await store.refresh()
        #expect(store.loading == .loaded && store.isAvailable && store.hasAnswered && !store.orgs.isEmpty)
    }

    @Test("the same when it is the account's read that is cancelled")
    func cancelledAccountRead() async {
        let store = makeStore()
        await store.refresh()
        source.set(me: .failure(CancellationError()))
        await store.refresh()
        #expect(store.loading == .loaded && store.isAvailable && !store.orgs.isEmpty)
    }

    @Test("a first read that is cancelled stays as it was before it, not answered")
    func cancelledFirstRead() async {
        source.set(outcomes: { _, _ in throw CancellationError() })
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .idle && !store.hasAnswered && !store.isAvailable)
    }

    @Test("a session that ends in the middle of the read asks for the sign-in again, and is not a failure of the numbers")
    func signInEndsInTheMiddle() async {
        let store = makeStore()
        await store.refresh()
        source.set(outcomes: { _, _ in throw NodeError.sessionExpired })
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.loading == .failed(Ja.Desk.signInEnded))
        #expect(store.isAvailable && !store.orgs.isEmpty, "what was read stays")
        source.set(outcomes: { _, _ in throw NodeError.notSignedIn })
        await store.refresh()
        #expect(store.needsSignIn)
        source.set(outcomes: { _, _ in try OutcomeSamples.outcomes() })
        await store.refresh()
        #expect(!store.needsSignIn && store.loading == .loaded)
    }

    @Test("an account with no orgs has answered with nothing, and the page is not offered")
    func noOrgs() async throws {
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded && store.hasAnswered && !store.isAvailable)
    }
}
