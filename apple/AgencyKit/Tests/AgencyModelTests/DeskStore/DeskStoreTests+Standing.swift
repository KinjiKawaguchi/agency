@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// What the node says besides the requests: recoveries, who is asking and what waits for the root key.
extension DeskStoreTests {
    @Test("a started recovery the node names is on the desk until the node stops naming it, and a failed read leaves it")
    func recoveries() async throws {
        func me(_ recovery: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[]\#(recovery)}]}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        #expect(store.recoveries.isEmpty)
        source.set(me: .success(try me(#","recovery":{"takesEffectAt":"2026-11-29T09:30:00Z"}"#)))
        await store.refresh()
        #expect(store.recoveries.map(\.orgID) == ["acme"])
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.recoveries.map(\.orgID) == ["acme"], "a failed read says nothing of it: it stays")
        source.set(me: .success(try me("")))
        await store.refresh()
        #expect(store.recoveries.isEmpty, "the node no longer says it waits: cancelled or taken")
    }

    @Test("what the node says the viewer is is known once the account has answered: a hosted member, or anyone else, and a later failed read keeps it")
    func standing() async throws {
        func me(_ extra: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]\#(extra)}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        #expect(store.standing == .loading, "not told yet is not anyone")
        source.set(me: .success(try me(#","guest":{"member":"erin","org":"acme"}"#)))
        await store.refresh()
        #expect(store.standing == .guest(GuestRef(member: "erin", org: "acme")))
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.standing.isGuest, "a read that fails later does not make the guest anyone else")
        let owner = makeStore()
        source.set(me: .success(try me("")))
        await owner.refresh()
        #expect(owner.standing == .member)
    }

    @Test("a store that replaces one of the same node and account starts from what was known, until the node says it again; a read that fails does not take it away")
    func standingSeeded() async throws {
        func me(_ extra: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]\#(extra)}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let seeded = makeStore(standing: .member)
        #expect(seeded.standing == .member, "the owner's open screen is not closed while the node is asked again")
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await seeded.refresh()
        #expect(seeded.standing == .member)
        source.set(me: .success(try me(#","guest":{"member":"erin","org":"acme"}"#)))
        await seeded.refresh()
        #expect(seeded.standing.isGuest, "what the node says now is what counts")
        #expect(makeStore().standing == .loading, "a store with nothing before it has not been told")
    }

    @Test("a read that fails with nothing earlier to go by counts as anyone else, and an ended sign-in says nothing of who is asking")
    func standingWhenFailed() async {
        let store = makeStore()
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.standing == .unknown)
        let signedOut = makeStore()
        source.set(me: .failure(NodeError.sessionExpired))
        await signedOut.refresh()
        #expect(signedOut.standing == .loading, "the sign-in has to run again before the node can say")
    }

    @Test("what waits for the root key is on the desk until the node stops naming it, a failed read leaves it, and nothing is there before the node answered")
    func rootChanges() async throws {
        func me(_ changes: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[]}]\#(changes)}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        #expect(store.rootChanges == nil)
        source.set(me: .success(try me(#","rootChanges":[{"id":"c1","org":"acme","what":"add member carol (Carol)","receivedAt":"2026-11-26T09:00:00Z"}]"#)))
        await store.refresh()
        #expect(store.rootChanges?.items.map(\.id) == ["c1"])
        source.set(me: .failure(NodeError.http(status: 503, message: "down")))
        await store.refresh()
        #expect(store.rootChanges?.items.map(\.id) == ["c1"], "a failed read says nothing of it: it stays")
        source.set(me: .success(try me("")))
        await store.refresh()
        #expect(store.rootChanges == nil, "the node no longer says it waits: signed or dropped")
    }

    @Test("a read that is cancelled after the account answered keeps the newer list of recoveries")
    func recoveriesKeptWhenCancelledAfterTheAccount() async throws {
        func me(_ recovery: String) throws -> Me {
            let json = #"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[{"id":"acme","isRoot":true,"label":"Acme","teams":[{"id":"app","label":"app","location":"local","viewer":"owner"}]\#(recovery)}]}"#
            return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
        }
        let store = makeStore()
        source.set(me: .success(try me("")))
        await store.refresh()
        #expect(store.recoveries.isEmpty)
        source.set(me: .success(try me(#","recovery":{}"#)))
        source.hold(team: "app")
        let reading = Task { await store.refresh() }
        while !source.isHoldingTeam { await Task.yield() }
        reading.cancel()
        source.releaseTeam()
        await reading.value
        #expect(store.recoveries.map(\.orgID) == ["acme"], "what the account said stays, whatever became of the rest of the read")
    }
}
