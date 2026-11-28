@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// Drafts, and what a decision does on the device and on the pile.
extension DeskStoreTests {
    // MARK: Drafts

    @Test("drafts are kept as they change, and an empty one is dropped")
    func drafts() {
        let store = makeStore()
        store.setText("もう少し待って", for: first)
        store.toggleOption("a", for: first)
        #expect(store.draft(for: first) == DeskDraft(text: "もう少し待って", optionKey: "a"))
        #expect(storage.drafts[first] == DeskDraft(text: "もう少し待って", optionKey: "a"))
        store.toggleOption("a", for: first)
        #expect(store.draft(for: first).optionKey == nil)
        store.setText("", for: first)
        #expect(store.drafts.isEmpty)
        #expect(storage.drafts.isEmpty)
    }

    @Test("drafts come back from the storage at the next launch")
    func draftsReload() {
        let kept = MemoryStorage([first: DeskDraft(text: "x", optionKey: "b")])
        #expect(makeStore(storage: kept).draft(for: first) == DeskDraft(text: "x", optionKey: "b"))
    }

    // MARK: Deciding

    @Test("a decision is signed as prepared, kept on the device, and the item leaves the pile and the count at once")
    func decide() async {
        let store = makeStore()
        await store.refresh()
        store.setText("条件つきで", for: first)
        submitter.hold()
        await store.send(.answer("条件つきで"), for: first)
        #expect(sender.sent.count == 1 && !sender.sent[0].isComment && sender.sent[0].text == "条件つきで" && sender.sent[0].answerWith.target == "r-1")
        #expect(store.draft(for: first).isEmpty && storage.drafts.isEmpty)
        #expect(kept.entries.map(\.itemID) == [first], "on the device before anything is sent")
        #expect(!store.items.map(\.id).contains(first), "off the pile")
        #expect(store.waitingCount == 1, "and off the count: the other one still waits")
        #expect(store.sending.isEmpty)
        submitter.release()
    }

    @Test("deciding does not wait for the node: the node never answers and the owner is on the next item")
    func doesNotWait() async {
        let store = makeStore()
        await store.refresh()
        submitter.hold()
        await store.send(.answer("go"), for: first)
        await waitUntil { submitter.isHolding }
        #expect(store.outbox.sending.count == 1, "it is being sent, in the background")
        #expect(store.items.map(\.id) == ["acme:app:r-2"] && store.sending.isEmpty)
        submitter.release()
    }

    @Test("once the node has taken it the entry is gone, and the desk is read again")
    func taken() async {
        let store = makeStore()
        await store.refresh()
        let readsBefore = source.meCalls
        await store.send(.answer("yes"), for: first)
        await store.outbox.flush()
        #expect(store.outbox.entries.isEmpty && kept.entries.isEmpty)
        #expect(submitter.sent.map(\.envelope.id) == ["env-1"])
        #expect(source.meCalls == readsBefore + 1)
    }

    @Test("a decision the node took stays off the pile and the count until the desk has been read again: the request does not flash back")
    func takenStaysOffUntilRead() async {
        let store = makeStore()
        await store.refresh()
        source.holdNextMe()
        await store.send(.answer("yes"), for: first)
        await waitUntil { source.isHolding }
        #expect(store.outbox.entries.isEmpty, "the node took it, and the desk is being read again")
        #expect(!store.items.map(\.id).contains(first) && store.waitingCount == 1)
        source.release()
        for _ in 0..<200_000 where !store.outbox.justTaken.isEmpty { await Task.yield() }
        #expect(store.outbox.justTaken.isEmpty, "and it is let go of once the desk has been read")
    }

    @Test("a decided item does not come back on the pile while its send is pending or refused, though the node's view still lists it, and comes back when discarded")
    func staysOff() async {
        let store = makeStore()
        await store.refresh()
        submitter.set(failure: NodeError.http(status: 409, message: "refused: not yours"))
        await store.send(.answer("hm"), for: first)
        await store.outbox.flush()
        await store.refresh()
        #expect(!store.items.map(\.id).contains(first), "refused for good: it waits for the owner to let go")
        #expect(store.outbox.failed.first?.failure?.reason == "refused: not yours")
        let id = store.outbox.entries[0].id
        store.discardDecision(id)
        #expect(store.items.map(\.id).contains(first) && store.waitingCount == 2)
        #expect(store.draft(for: first) == DeskDraft(text: "hm", optionKey: nil), "what was typed is back in the draft")
    }

    private func log(_ state: String, target: String = "r-1") -> DecisionLog {
        let json = """
        {"entries":[{"choice":"a","decidedAt":"2026-11-27T10:00:00Z","id":"env-1","kind":"answer","org":"acme","state":\(state),
        "target":"\(target)","team":"app","text":""}]}
        """
        return try! JSONDecoder().decode(DecisionLog.self, from: Data(json.utf8))
    }

    @Test("a decision the node took stays off the pile and the count while the decision log says it is queued, though the node's view still lists the request")
    func queuedStaysOff() async {
        let store = makeStore()
        await store.send(.answer("yes"), for: first)
        await store.refresh()
        source.set(decisions: .success(log(#"{"kind":"queued","attempts":1,"nextAt":"2026-11-27T10:00:15Z"}"#)))
        await store.outbox.flush()
        await store.refresh()
        #expect(store.outbox.entries.isEmpty, "the node took it")
        #expect(!store.items.map(\.id).contains(first) && store.waitingCount == 1, "not back before the team has applied it")
        source.set(decisions: .success(log(#"{"kind":"applied"}"#)))
        await store.refresh()
        #expect(store.items.map(\.id).contains(first), "the node's own view decides once the log no longer says queued")
    }

    @Test("a decision the team's node refused later is back on the pile")
    func refusedLaterIsBack() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.answer("yes"), for: first)
        await store.outbox.flush()
        source.set(decisions: .success(log(#"{"kind":"refused","message":"not yours"}"#)))
        await store.refresh()
        #expect(store.items.map(\.id).contains(first))
    }

    @Test("a comment on a request with options is kept as a comment: no choice, and nothing decided")
    func decideComment() async {
        source.set { _, _ in Teams.snapshot() }
        let store = makeStore()
        await store.refresh()
        let question = "acme:app:" + Teams.questionID
        await store.send(.comment("来週にしてください"), for: question)
        #expect(sender.sent.count == 1 && sender.sent[0].isComment && sender.sent[0].choice == nil)
        #expect(store.outbox.entries.first?.isComment == true)
    }

    @Test("a decision that cannot be signed keeps the draft and says why; editing clears the message")
    func signFails() async {
        let store = makeStore()
        await store.refresh()
        store.setText("keep me", for: first)
        sender.set(failure: NodeError.http(status: 403, message: "writes are for the owner"))
        await store.send(.answer("keep me"), for: first)
        #expect(store.sendFailures[first] == "writes are for the owner")
        #expect(store.draft(for: first).text == "keep me")
        #expect(store.outbox.entries.isEmpty && store.items.map(\.id).contains(first))
        #expect(store.sending.isEmpty)
        store.setText("keep me!", for: first)
        #expect(store.sendFailures[first] == nil)
    }

    @Test("a decision that cannot be written to the device is not accepted: the draft stays and the item stays")
    func notKept() async {
        let store = makeStore()
        await store.refresh()
        store.setText("x", for: first)
        kept.failsToWrite = true
        await store.send(.answer("x"), for: first)
        #expect(store.sendFailures[first] == Ja.Outbox.notKept && store.draft(for: first).text == "x")
        #expect(store.outbox.entries.isEmpty && store.items.map(\.id).contains(first) && submitter.sent.isEmpty)
    }

    @Test("a decision that finds the session over asks for a new sign-in and keeps the draft")
    func signSessionEnded() async {
        let store = makeStore()
        await store.refresh()
        store.setText("x", for: first)
        sender.set(failure: NodeError.sessionExpired)
        await store.send(.answer("x"), for: first)
        #expect(store.needsSignIn)
        #expect(store.draft(for: first).text == "x")
    }

    @Test("a reply the request does not allow is refused before anything is signed")
    func refusedBeforeSigning() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.comment("words"), for: first)
        #expect(sender.sent.isEmpty)
        #expect(store.sendFailures[first] != nil)
        await store.send(.comment("x"), for: "acme:app:unknown")
        #expect(sender.sent.isEmpty)
    }

    @Test("while a decision is being signed, the same item is not decided again")
    func noDoubleDecision() async {
        let store = makeStore()
        await store.refresh()
        sender.holdNextSend()
        let running = Task { await store.send(.answer("once"), for: first) }
        await waitUntil { sender.isHolding }
        #expect(store.sending == [first])
        await store.send(.answer("twice"), for: first)
        sender.release()
        await running.value
        #expect(sender.sent.count == 1 && sender.sent[0].text == "once")
    }

    @Test("an item that was already decided is not decided again")
    func noRedecision() async {
        let store = makeStore()
        await store.refresh()
        await store.send(.answer("one"), for: first)
        await store.send(.answer("two"), for: first)
        #expect(sender.sent.count == 1)
    }
}
