@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// What the desk does when the node cannot answer, and what it tells the cache after a decision.
extension DeskStoreTests {
    @Test("after the node has taken a decision the cache is told, before the desk reads again")
    func tellsTheCache() async {
        let told = Told()
        let store = DeskStore(
            source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage,
            afterSend: { await told.mark(readsSoFar: source.meCalls) }, retry: .never
        )
        await store.refresh()
        let before = source.meCalls
        await store.send(.answer("yes"), for: first)
        await store.outbox.flush()
        #expect(await told.count == 1)
        #expect(await told.readsAtThatTime == before, "told before the second read")
        #expect(source.meCalls == before + 1)
    }

    @Test("an ended Access session asks for a new sign-in and keeps what was on the desk")
    func sessionEnded() async {
        let store = makeStore()
        await store.refresh()
        source.set(me: .failure(NodeError.sessionExpired))
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.items.count == 2)
        if case .failed = store.loading {} else { Issue.record("loading should have failed") }
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(!store.needsSignIn)
        #expect(store.loading == .loaded)
    }

    @Test("no Access token yet asks for a sign-in the same way")
    func notSignedIn() async {
        source.set(me: .failure(NodeError.notSignedIn))
        let store = makeStore()
        await store.refresh()
        #expect(store.needsSignIn)
        #expect(store.items.isEmpty)
    }

    @Test("another failure shows the node's reason and keeps the desk")
    func otherFailure() async {
        let store = makeStore()
        await store.refresh()
        source.set(me: .failure(NodeError.http(status: 502, message: nil)))
        await store.refresh()
        #expect(store.loading == .failed("HTTP 502"))
        #expect(!store.needsSignIn)
        #expect(store.items.count == 2)
    }

    @Test("recent decisions that cannot be read are reported without failing the desk")
    func recentFailure() async {
        source.set(decisions: .failure(NodeError.http(status: 500, message: "boom")))
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.recentFailure == "boom")
        source.set(decisions: .success(Fixture.decode(DecisionLog.self, "decision-log")))
        await store.refresh()
        #expect(store.recentFailure == nil)
        #expect(store.recent.count == 3)
    }
}
