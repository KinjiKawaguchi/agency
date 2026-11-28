@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// What the desk makes of one read: what it knows, what it keeps and when it has answered.
extension DeskStoreTests {
    @Test("refresh reads the teams and the recent decisions, and counts what waits on the owner")
    func refresh() async {
        let store = makeStore()
        #expect(store.loading == .idle)
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.items.map(\.id) == ["acme:app:r-1", "acme:app:r-2"])
        #expect(store.waitingCount == 2)
        #expect(store.recent.count == 3)
        #expect(store.teams.count == 1)
        #expect(!store.needsSignIn)
    }

    @Test("before the node has answered there is no answer: nothing waits is not claimed, and the count is unknown")
    func beforeAnswer() async {
        let store = makeStore()
        #expect(!store.hasAnswer)
        #expect(store.knownWaitingCount == nil)
        await store.refresh()
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 2)
    }

    @Test("a read that fails before any answer is no answer, and one that fails after an answer keeps it")
    func failedRead() async {
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        let store = makeStore()
        await store.refresh()
        #expect(!store.hasAnswer)
        #expect(store.knownWaitingCount == nil)
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(store.knownWaitingCount == 2)
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.loading != .loaded)
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 2)
    }

    @Test("a node that answers with nothing to read has answered: the count is 0, not unknown")
    func answeredWithNothing() async throws {
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        let store = makeStore()
        await store.refresh()
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 0)
    }

    @Test("once a read has finished the desk stays answered: an empty desk keeps its 0 while a refresh is under way and after a failed one")
    func answeredStays() async throws {
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        let store = makeStore()
        #expect(store.knownWaitingCount == nil)
        await store.refresh()
        #expect(store.knownWaitingCount == 0)
        #expect(store.hasAnswer)

        source.holdNextMe()
        let refreshing = Task { await store.refresh() }
        while !source.isHolding { await Task.yield() }
        #expect(store.loading == .loading)
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 0)
        source.release()
        await refreshing.value

        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.loading != .loaded)
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == 0)
    }

    @Test("a failed read is remembered: it stays set while the next refresh is under way, and a read that finishes whole clears it")
    func readFailing() async throws {
        let store = makeStore()
        #expect(!store.isReadFailing)
        await store.refresh()
        #expect(!store.isReadFailing)

        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.isReadFailing)

        source.set(me: .success(Fixture.decode(Me.self, "me")))
        source.holdNextMe()
        let refreshing = Task { await store.refresh() }
        while !source.isHolding { await Task.yield() }
        #expect(store.loading == .loading)
        #expect(store.isReadFailing, "a refresh under way is not a read that worked")
        source.release()
        await refreshing.value
        #expect(!store.isReadFailing)
    }

    @Test("a read that was cancelled says nothing about the node: no failure, and what was known stays", arguments: [
        CancellationError() as Error, URLError(.cancelled),
    ])
    func cancelledRead(error: Error) async {
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded && !store.isReadFailing)
        source.set(me: .failure(error))
        await store.refresh()
        #expect(!store.isReadFailing)
        #expect(store.loading == .loaded, "the desk still shows what it read")
        #expect(store.hasAnswer)
        let first = makeStore()
        source.set(me: .failure(error))
        await first.refresh()
        #expect(first.loading == .idle && !first.isReadFailing, "a first read that was cancelled has not failed either")
    }

    @Test("an expired sign-in and a sign-in that was never made are failed reads too")
    func readFailingForTheSignIn() async {
        let store = makeStore()
        source.set(me: .failure(NodeError.sessionExpired))
        await store.refresh()
        #expect(store.isReadFailing)
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        await store.refresh()
        #expect(!store.isReadFailing)
        source.set(me: .failure(NodeError.notSignedIn))
        await store.refresh()
        #expect(store.isReadFailing)
    }

    @Test("a read that finished with a team that could not be read is a failing read: the number is not the whole desk")
    func partialReadIsFailing() async throws {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"remote","viewer":"owner"}]}]}
        """
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(two.utf8))))
        source.set { _, team in
            if team == "web" { throw NodeError.http(status: 503, message: "no replica yet") }
            return Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
        }
        let store = makeStore()
        await store.refresh()
        #expect(store.loading == .loaded)
        #expect(store.isReadFailing)
        source.set { _, _ in Fixture.decode(TeamSnapshot.self, "team-snapshot-local") }
        await store.refresh()
        #expect(!store.isReadFailing)
    }
}
