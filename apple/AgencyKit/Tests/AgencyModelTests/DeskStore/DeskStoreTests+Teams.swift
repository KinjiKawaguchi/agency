@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// How a team is read: one that fails, one that left, one that is not ready, one that answers late.
extension DeskStoreTests {
    @Test("during the first read a team that has answered is shown, and the number is still unknown until the read has finished")
    func firstReadPartial() async throws {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"local","viewer":"owner"}]}]}
        """
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(two.utf8))))
        source.hold(team: "web")
        let store = makeStore()
        let reading = Task { await store.refresh() }
        // Both teams are read at once: wait until `web` is held and `app` has been shown.
        while !(source.isHoldingTeam && !store.teams.isEmpty) { await Task.yield() }
        #expect(store.hasAnswer)
        #expect(store.knownWaitingCount == nil)
        source.releaseTeam()
        await reading.value
        #expect(store.knownWaitingCount != nil)
    }

    @Test("a team that cannot be read does not hide the others")
    func teamFailure() async throws {
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
        #expect(store.items.count == 2)
        #expect(store.teamFailures == ["acme/web": "no replica yet"])
    }

    @Test("a team that was read keeps its last answer when a later read of it fails, and the failure is reported")
    func keepsLastAnswer() async {
        let store = makeStore()
        await store.refresh()
        #expect(store.items.count == 2)
        source.set { _, _ in throw NodeError.http(status: 503, message: "not ready") }
        await store.refresh()
        #expect(store.items.count == 2, "the desk keeps what it showed")
        #expect(store.teamFailures == ["acme/app": "not ready"])
        source.set { _, _ in Fixture.decode(TeamSnapshot.self, "team-snapshot-local") }
        await store.refresh()
        #expect(store.teamFailures.isEmpty)
    }

    @Test("a team the account no longer lists is dropped, not kept")
    func dropsTeamsThatLeft() async throws {
        let store = makeStore()
        await store.refresh()
        let empty = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(empty.utf8))))
        await store.refresh()
        #expect(store.items.isEmpty)
        #expect(store.teams.isEmpty)
    }

    @Test("a team that left and comes back with a failing read does not bring its old answer back")
    func leftTeamIsForgotten() async throws {
        let store = makeStore()
        await store.refresh()
        let none = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(none.utf8))))
        await store.refresh()
        source.set(me: .success(Fixture.decode(Me.self, "me")))
        source.set { _, _ in throw NodeError.http(status: 503, message: "not ready") }
        await store.refresh()
        #expect(store.items.isEmpty, "the old answer of a team that left must not come back")
        #expect(store.teamFailures == ["acme/app": "not ready"])
    }

    @Test("a team whose view is not ready is asked again, and shown once it answers, with nothing reported")
    func asksAgain() async {
        let tries = Counter()
        source.set { _, _ in
            if tries.next() <= 2 { throw NodeError.http(status: 503, message: "the board has not been read yet") }
            return Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
        }
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: RetryPolicy(delays: [1, 1], wait: { _ in }))
        await store.refresh()
        #expect(tries.count == 3)
        #expect(store.items.count == 2)
        #expect(store.teamFailures.isEmpty)
        #expect(store.loading == .loaded)
    }

    @Test("a team that keeps failing is reported after the waits are used up, and keeps its last answer")
    func givesUp() async {
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: RetryPolicy(delays: [1, 1], wait: { _ in }))
        await store.refresh()
        let tries = Counter()
        source.set { _, _ in tries.next(); throw NodeError.http(status: 503, message: "not ready") }
        await store.refresh()
        #expect(tries.count == 3)
        #expect(store.teamFailures == ["acme/app": "not ready"])
        #expect(store.items.count == 2)
    }

    @Test("a team that answers is shown while another is still being asked again")
    func showsWhileWaiting() async throws {
        let two = """
        {"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[
        {"id":"app","label":"app","location":"local","viewer":"owner"},{"id":"web","label":"web","location":"remote","viewer":"owner"}]}]}
        """
        source.set(me: .success(try JSONDecoder().decode(Me.self, from: Data(two.utf8))))
        let webTries = Counter()
        source.set { _, team in
            if team == "web", webTries.next() == 1 { throw NodeError.http(status: 503, message: "not ready") }
            return Fixture.decode(TeamSnapshot.self, "team-snapshot-local")
        }
        let holder = StoreHolder()
        let shownAtTheWait = Counter()
        // The wait before web is asked again: it looks at the desk, giving app time to answer first.
        let policy = RetryPolicy(delays: [1], wait: { _ in
            for _ in 0..<1000 {
                if await MainActor.run(body: { holder.store?.teams.count ?? 0 }) == 1 { break }
                try await Task.sleep(for: .milliseconds(1))
            }
            shownAtTheWait.set(await MainActor.run { holder.store?.teams.count ?? -1 })
        })
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: policy)
        holder.store = store
        await store.refresh()
        #expect(shownAtTheWait.count == 1, "app was on the desk while web was waited for")
        #expect(store.teams.count == 2)
    }

    @Test("an answer that is final is not asked again")
    func finalAnswer() async {
        let tries = Counter()
        source.set { _, _ in tries.next(); throw NodeError.http(status: 403, message: "no") }
        let store = DeskStore(source: source, signer: sender, outbox: DecisionOutbox(storage: kept, submitter: submitter), storage: storage, retry: RetryPolicy(delays: [1, 1], wait: { _ in }))
        await store.refresh()
        #expect(tries.count == 1)
    }
}
