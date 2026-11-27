@testable import AgencyAPI
@testable import AgencyModel
import AgencyProtocol
import Foundation
import Testing

/// Three devices with real keys: this one, a phone and a laptop.
private enum Keys {
    static let own = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32))
    static let phone = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0c, count: 32))
    static let laptop = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0d, count: 32))
    static func id(_ signer: SoftwareP256Signer) -> String { signer.publicKey.fingerprint.deviceID }
}

private func device(_ signer: SoftwareP256Signer, _ label: String, at: String = "2026-11-24T12:00:00Z") -> String {
    #"{"addedAt":"\#(at)","id":"\#(Keys.id(signer))","label":"\#(label)","publicKey":"\#(signer.publicKey.string)"}"#
}

private func list(_ orgs: [(String, [String])]) -> DeviceList {
    let body = orgs.map { #"{"org":"\#($0.0)","devices":[\#($0.1.joined(separator: ","))]}"# }.joined(separator: ",")
    return try! JSONDecoder().decode(DeviceList.self, from: Data(#"{"orgs":[\#(body)],"requests":[]}"#.utf8))
}

private func plan(for signer: SoftwareP256Signer, in devices: DeviceList) throws -> DeviceRemovalPlan {
    let row = try #require(DeviceFacts.rows(devices, ownKey: Keys.own.publicKey.string).first { $0.publicKey == signer.publicKey.string })
    return DeviceRemovalPlan(for: row, in: devices, ownKey: Keys.own.publicKey.string)
}

@Suite("Device removal plan")
struct DeviceRemovalPlanTests {
    private let both = [device(Keys.own, "This phone"), device(Keys.phone, "Old phone")]

    @Test("another device that is not the person's last in the org is removed there, signed by this device as its own id, with the id as the node gave it")
    func oneOrg() throws {
        let removal = try plan(for: Keys.phone, in: list([("acme", both)]))
        #expect(removal.targets == [.init(org: "acme", deviceID: Keys.id(Keys.phone), signer: Keys.id(Keys.own))])
        #expect(!removal.isBlocked && removal.canConfirm && !removal.isThisDevice && removal.stillIn.isEmpty && removal.lastIn.isEmpty)
        #expect(removal.title == "Old phoneを削除しますか?")
        #expect(removal.message == "Old phone(照合コード \(Keys.phone.publicKey.fingerprint.pairingCode))は、二度と署名できなくなります。")
    }

    @Test("a device in several orgs is removed in each with one confirmation, which names the orgs")
    func severalOrgs() throws {
        let removal = try plan(for: Keys.phone, in: list([("acme", both), ("beta", both)]))
        #expect(removal.targets.map(\.org) == ["acme", "beta"])
        #expect(removal.confirmation.last == "acme、beta から削除されます。")
    }

    @Test("the person's last device in an org cannot be removed from a client, and if it holds in any org nothing is removed in any")
    func lastDevice() throws {
        let alone = try plan(for: Keys.phone, in: list([("acme", [device(Keys.phone, "Old phone")])]))
        #expect(alone.isBlocked && alone.targets.isEmpty && alone.lastIn == ["acme"] && !alone.canConfirm)
        #expect(alone.message == "Old phoneは、acmeでのただ 1 つの端末です。削除すると、そこでは誰も署名できなくなるため、ここでは削除できません。先に別の端末を追加してください。このノードを動かしているマシンで agency device-add を実行しても、追加できます。")
        let oneOfTwo = try plan(for: Keys.phone, in: list([("acme", both), ("beta", [device(Keys.own, "This phone"), device(Keys.phone, "Old phone")]), ("lab", [device(Keys.phone, "Old phone")])]))
        #expect(oneOfTwo.isBlocked && oneOfTwo.targets.isEmpty && oneOfTwo.lastIn == ["lab"], "no part of it is removed")
    }

    @Test("an org where this device is not added is not signed for: the device stays there, the person is told, and the others are removed")
    func notAddedHere() throws {
        let devices = list([("acme", both), ("beta", [device(Keys.phone, "Old phone"), device(Keys.laptop, "Laptop")])])
        let removal = try plan(for: Keys.phone, in: devices)
        #expect(removal.targets.map(\.org) == ["acme"] && removal.stillIn == ["beta"] && !removal.isBlocked)
        #expect(removal.confirmation.last == "beta にはまだ載っています。この端末はそこに追加されていないため、削除に署名できません。")
        let nowhere = try plan(for: Keys.phone, in: list([("beta", [device(Keys.phone, "Old phone"), device(Keys.laptop, "Laptop")])]))
        #expect(nowhere.isBlocked && nowhere.targets.isEmpty && nowhere.message == "beta にはまだ載っています。この端末はそこに追加されていないため、削除に署名できません。")
    }

    @Test("this device is its own case: its sentence is added, it signs its own removal, and it is blocked when it is the only one")
    func thisDevice() throws {
        let removal = try plan(for: Keys.own, in: list([("acme", both)]))
        #expect(removal.isThisDevice && removal.targets.first?.signer == Keys.id(Keys.own) && removal.targets.first?.deviceID == Keys.id(Keys.own))
        #expect(removal.confirmation.last == Ja.Devices.Remove.thisDevice)
        #expect(Ja.Devices.Remove.thisDevice == "いま使っている端末です。削除すると、この端末はこの鍵では署名できなくなり、この鍵をもう一度追加することもできません。そのあと端末は新しい鍵を作り、それは新しい端末として追加する必要があります。")
        #expect(Ja.Devices.Remove.thisDeviceRemoved == "この端末を削除しました。この端末は新しい鍵を作りました。まだ追加されていません。また決めるには、この画面で追加を依頼してください。")
        #expect(Ja.Devices.Remove.keyNotMade("no key") == "端末は削除しましたが、この端末は新しい鍵を作れませんでした: no key")
        #expect(try plan(for: Keys.own, in: list([("acme", [device(Keys.own, "This phone")])])).isBlocked)
    }

    @Test("the words are the web's: the button, the states and what each outcome says")
    func words() {
        #expect(Ja.Devices.Remove.button == "削除" && Ja.Devices.Remove.buttonFor("Old phone") == "Old phoneを削除")
        #expect(Ja.Devices.Remove.confirm == "この端末を削除する" && Ja.Devices.Remove.cancel == "キャンセル" && Ja.Devices.Remove.removing == "削除しています…")
        #expect(Ja.Devices.Remove.removed("Old phone") == "Old phoneを削除しました。二度と署名できません。")
        #expect(Ja.Devices.Remove.failed("no") == "端末を削除できませんでした: no")
        #expect(Ja.Devices.Remove.pendingRoot(command: "agency root-sign") == "ここでは削除を依頼しました。ルート鍵のノードが署名すると有効になります。そのノードで agency root-sign を実行してください。")
    }
}

private final class RemovalPoster: DeviceChangePoster, @unchecked Sendable {
    private let lock = NSLock()
    let devices: DeviceList
    var queue: [Result<DeviceChangeResult, Error>] = []
    private var _submitted: [SignedDeviceChange] = []
    private var times = 0
    init(_ devices: DeviceList) { self.devices = devices }
    var submitted: [SignedDeviceChange] { lock.withLock { _submitted } }
    func me() async throws -> Me { Fixture.decode(Me.self, "me") }
    func devices() async throws -> DeviceList { devices }
    func refreshSession() async throws -> Session { try JSONDecoder().decode(Session.self, from: Data(#"{"writeToken":"w","now":"2026-11-24T12:00:00Z"}"#.utf8)) }
    func nodeNow() async -> Date { lock.withLock { times += 1; return Date(timeIntervalSince1970: 1_795_521_600 + Double(times)) } }
    func submitDeviceChange(_ change: SignedDeviceChange) async throws -> DeviceChangeResult {
        let next = lock.withLock { () -> Result<DeviceChangeResult, Error> in
            _submitted.append(change)
            return queue.isEmpty ? .success(DeviceChangeResult(outcome: .applied, message: nil)) : queue.removeFirst()
        }
        return try next.get()
    }
}

private final class ReasonSigner: DeviceSigner, @unchecked Sendable {
    private let lock = NSLock()
    private var _reasons: [String] = []
    var publicKey: DevicePublicKey { Keys.own.publicKey }
    var reasons: [String] { lock.withLock { _reasons } }
    func sign(_ text: Data, reason: String) async throws -> Data {
        lock.withLock { _reasons.append(reason) }
        return try await Keys.own.sign(text, reason: reason)
    }
    /// Asks once for all of them, as the app's signer does.
    func signEach(_ texts: [Data], reason: String) async throws -> [Data] {
        lock.withLock { _reasons.append(reason) }
        return try await texts.asyncMap { try await Keys.own.sign($0, reason: reason) }
    }
}

@Suite("Device removal sender")
struct DeviceRemovalSenderTests {
    private let both = [device(Keys.own, "This phone"), device(Keys.phone, "Old phone")]

    @Test("a removal is a change for each org, signed by this device as its own id there, over the device's id as the node gave it, after Face ID naming it")
    func signs() async throws {
        let devices = list([("acme", both), ("beta", both)])
        let poster = RemovalPoster(devices)
        let signer = ReasonSigner()
        let results = try await DeviceChangeSender(poster: poster, signer: signer).remove(try plan(for: Keys.phone, in: devices))
        #expect(results.map(\.outcome) == [.applied, .applied])
        #expect(poster.submitted.map(\.change.org) == ["acme", "beta"] && poster.submitted.allSatisfy { $0.signer == Keys.id(Keys.own) })
        for sent in poster.submitted {
            guard case .remove(let id) = sent.change.operation else { Issue.record("should be a removal"); return }
            #expect(id == Keys.id(Keys.phone))
            let text = try signedText(kind: .device, body: sent.change.json)
            #expect(Keys.own.publicKey.verifies(try #require(Data(base64Encoded: sent.signature)), over: text))
        }
        #expect(signer.reasons == ["端末を削除する: Old phone"], "one confirmation for what the confirmation listed, not one for each org")
    }

    @Test("a removal that cannot be made signs nothing and asks for nothing")
    func blocked() async throws {
        let devices = list([("acme", [device(Keys.phone, "Old phone")])])
        let poster = RemovalPoster(devices)
        let signer = ReasonSigner()
        await #expect(throws: DeviceChangeSenderError.nowhereToRemove) { _ = try await DeviceChangeSender(poster: poster, signer: signer).remove(try plan(for: Keys.phone, in: devices)) }
        #expect(poster.submitted.isEmpty && signer.reasons.isEmpty)
    }

    @Test("a later org's failure keeps what the node answered for the earlier ones, and the first org's failure is the failure itself")
    func partly() async throws {
        let devices = list([("acme", both), ("beta", both)])
        let poster = RemovalPoster(devices)
        poster.queue = [.success(DeviceChangeResult(outcome: .pending, message: nil)), .failure(NodeError.http(status: 409, message: "no"))]
        do {
            _ = try await DeviceChangeSender(poster: poster, signer: ReasonSigner()).remove(try plan(for: Keys.phone, in: devices))
            Issue.record("should fail")
        } catch let partly as DeviceChangePartlyAnswered {
            #expect(partly.answered.map(\.outcome) == [.pending])
        }
        let first = RemovalPoster(devices)
        first.queue = [.failure(NodeError.http(status: 409, message: "no"))]
        await #expect(throws: NodeError.http(status: 409, message: "no")) { _ = try await DeviceChangeSender(poster: first, signer: ReasonSigner()).remove(try plan(for: Keys.phone, in: devices)) }
    }
}

private final class ListSource: DevicesSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _list: DeviceList
    private var reads = 0
    private var failsFrom = Int.max
    init(_ list: DeviceList) { _list = list }
    func set(_ list: DeviceList) { lock.withLock { _list = list } }
    /// The node cannot be asked from the `read`th time on (the first read is 1).
    func fail(from read: Int) { lock.withLock { failsFrom = read } }
    func devices() async throws -> DeviceList {
        try lock.withLock {
            reads += 1
            if reads >= failsFrom { throw NodeError.http(status: 502, message: nil) }
            return _list
        }
    }
    func heldChanges() async throws -> [RootChangeRef] { [] }
}

private struct NoAdd: DeviceAdder {
    func add(_ request: DeviceRequest) async throws -> [DeviceChangeResult] { throw NodeError.notSignedIn }
}

private final class ScriptedRemover: DeviceRemover, @unchecked Sendable {
    private let lock = NSLock()
    private var _result: Result<[DeviceChangeResult], Error> = .success([DeviceChangeResult(outcome: .applied, message: nil)])
    private var _plans: [DeviceRemovalPlan] = []
    private var gate: CheckedContinuation<Void, Never>?
    private var holds = false
    var plans: [DeviceRemovalPlan] { lock.withLock { _plans } }
    var isHolding: Bool { lock.withLock { gate != nil } }
    func set(_ result: Result<[DeviceChangeResult], Error>) { lock.withLock { _result = result } }
    func hold() { lock.withLock { holds = true } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    func remove(_ plan: DeviceRemovalPlan) async throws -> [DeviceChangeResult] {
        lock.withLock { _plans.append(plan) }
        if lock.withLock({ holds }) { await withCheckedContinuation { c in lock.withLock { gate = c } } }
        return try lock.withLock { try _result.get() }.map { $0 }
    }
}

@MainActor
@Suite("Device removal store")
struct DeviceRemovalStoreTests {
    private let both = [device(Keys.own, "This phone"), device(Keys.phone, "Old phone")]

    private func store(_ source: ListSource, _ remover: ScriptedRemover?) -> DevicesStore {
        DevicesStore(source: source, adder: NoAdd(), ownKey: Keys.own.publicKey, remover: remover)
    }

    private func row(_ store: DevicesStore, _ signer: SoftwareP256Signer) throws -> DeviceFacts.Row {
        try #require(store.rows.first { $0.publicKey == signer.publicKey.string })
    }

    @Test("a removal is offered only where this build can make one and the node's list has been read")
    func offered() async throws {
        let source = ListSource(list([("acme", both)]))
        let without = store(source, nil)
        await without.refresh()
        #expect(!without.offersRemoval)
        let with = store(source, ScriptedRemover())
        #expect(!with.offersRemoval, "not before the list is read")
        await with.refresh()
        let target = try row(with, Keys.phone)
        #expect(with.offersRemoval && with.removalPlan(for: target)?.canConfirm == true)
    }

    @Test("a removal that took effect says so, and the device leaves the list when the node stops listing it, not before")
    func applied() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        await store.remove(target)
        #expect(remover.plans.count == 1 && remover.plans[0].targets.map(\.org) == ["acme"])
        #expect(store.removedNotice == "Old phoneを削除しました。二度と署名できません。" && !store.didRemoveThisDevice)
        #expect(store.rows.contains { $0.id == target.id }, "the node still lists it: it is not taken off the list on its own say")
        source.set(list([("acme", [device(Keys.own, "This phone")])]))
        await store.refresh()
        #expect(!store.rows.contains { $0.id == target.id })
        store.dismissRemovedNotice()
        #expect(store.removedNotice == nil)
    }

    @Test("a device stays on the list while the node still lists it, even when the list cannot be read again right after the removal")
    func staysUntilTheNodeSaysSo() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        source.fail(from: 3)
        await store.remove(target)
        #expect(store.removedNotice != nil && store.rows.contains { $0.id == target.id } && store.loadFailure != nil)
    }

    @Test("the list is read again before anything is signed: one that cannot be read, or that is not the one confirmed, signs nothing")
    func staleList() async throws {
        let source = ListSource(list([("acme", both), ("beta", both)]))
        let remover = ScriptedRemover()
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        source.set(list([("acme", both), ("beta", [device(Keys.phone, "Old phone")])]))
        await store.remove(target)
        #expect(remover.plans.isEmpty && store.removalFailures[target.id] == Ja.Devices.Remove.listChanged, "the last device rule is applied to the list as it is now")
        #expect(store.removalPlan(for: try row(store, Keys.phone))?.lastIn == ["beta"], "and the list is read again for the person to confirm again")
        source.fail(from: 1)
        await store.remove(target)
        #expect(remover.plans.isEmpty && store.removalFailures[target.id]?.hasPrefix("端末を削除できませんでした") == true)
    }

    @Test("this device is not removed while decisions signed with its key are kept on it; another device is")
    func keptDecisions() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        let store = DevicesStore(source: source, adder: NoAdd(), ownKey: Keys.own.publicKey, remover: remover, hasKeptDecisions: { true })
        await store.refresh()
        let own = try row(store, Keys.own)
        #expect(store.removalPlan(for: own)?.canConfirm == false && store.removalPlan(for: own)?.message == Ja.Devices.Remove.keptDecisions)
        await store.remove(own)
        #expect(remover.plans.isEmpty && store.removalFailures[own.id]?.contains("送れていない決定") == true)
        #expect(store.removalPlan(for: try row(store, Keys.phone))?.canConfirm == true)
        await store.remove(try row(store, Keys.phone))
        #expect(remover.plans.count == 1)
    }

    @Test("a new key is made only when this device was removed in every org: not while one holds it, and not when one failed")
    func newKeyOnlyWhenComplete() async throws {
        let twoOrgs = list([("acme", both), ("beta", both)])
        let held = ScriptedRemover()
        held.set(.success([DeviceChangeResult(outcome: .applied, message: nil), DeviceChangeResult(outcome: .pending, message: nil)]))
        let heldStore = store(ListSource(twoOrgs), held)
        await heldStore.refresh()
        await heldStore.remove(try row(heldStore, Keys.own))
        #expect(!heldStore.didRemoveThisDevice)
        let failed = ScriptedRemover()
        failed.set(.failure(DeviceChangePartlyAnswered(answered: [DeviceChangeResult(outcome: .applied, message: nil)], cause: NodeError.http(status: 500, message: "no"))))
        let failedStore = store(ListSource(twoOrgs), failed)
        await failedStore.refresh()
        await failedStore.remove(try row(failedStore, Keys.own))
        #expect(!failedStore.didRemoveThisDevice && failedStore.removalFailures.count == 1)
    }

    @Test("a removal the root's node holds is said as asked and kept on the device's row, and is not asked for again")
    func pending() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        remover.set(.success([DeviceChangeResult(outcome: .pending, message: "ignored")]))
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        await store.remove(target)
        #expect(store.removalNotes[target.id] == "ここでは削除を依頼しました。ルート鍵のノードが署名すると有効になります。そのノードで agency root-sign を実行してください。")
        #expect(store.removedNotice == nil && !store.didRemoveThisDevice)
        await store.remove(target)
        #expect(remover.plans.count == 1)
        source.set(list([("acme", [device(Keys.own, "This phone")])]))
        await store.refresh()
        #expect(store.removalNotes.isEmpty, "the note goes with the device")
    }

    @Test("an outcome the app does not know is never a removal: said with the node's message, or without one that it is not known and the list shows the result")
    func unknown() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        remover.set(.success([DeviceChangeResult(outcome: .unknown("later"), message: "It will take a while.")]))
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        await store.remove(target)
        #expect(store.removalNotes[target.id] == "It will take a while." && store.removedNotice == nil)
        #expect(Ja.Devices.unknownOutcome == "ノードが、このアプリの知らない答えを返しました。結果は端末の一覧で確かめてください。")
        let silent = ScriptedRemover()
        silent.set(.success([DeviceChangeResult(outcome: .unknown("later"), message: nil)]))
        let other = self.store(source, silent)
        await other.refresh()
        let silentTarget = try row(other, Keys.phone)
        await other.remove(silentTarget)
        #expect(other.removalNotes[silentTarget.id] == Ja.Devices.unknownOutcome && other.removedNotice == nil && !other.didRemoveThisDevice)
        source.set(list([("acme", [device(Keys.own, "This phone")])]))
        await other.refresh()
        #expect(other.rows.count == 1 && other.removalNotes.isEmpty, "the list is read again and shows the result")
        let own = ScriptedRemover()
        own.set(.success([DeviceChangeResult(outcome: .unknown("later"), message: nil)]))
        let ownStore = self.store(ListSource(list([("acme", both)])), own)
        await ownStore.refresh()
        await ownStore.remove(try row(ownStore, Keys.own))
        #expect(!ownStore.didRemoveThisDevice, "no new key is made for an answer that is not known")
    }

    @Test("a refusal says why, can be tried again, and a later org's failure keeps what an earlier one held")
    func failure() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        remover.set(.failure(NodeError.http(status: 409, message: "no such device")))
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        await store.remove(target)
        #expect(store.removalFailures[target.id] == "端末を削除できませんでした: no such device" && store.removedNotice == nil)
        remover.set(.failure(DeviceChangePartlyAnswered(answered: [DeviceChangeResult(outcome: .pending, message: nil)], cause: NodeError.http(status: 403, message: "refused"))))
        await store.remove(target)
        #expect(store.removalNotes[target.id] != nil && store.removalFailures[target.id] == "端末を削除できませんでした: refused")
    }

    @Test("a removal the client does not make is refused in words and nothing is asked of the node")
    func blocked() async throws {
        let source = ListSource(list([("acme", [device(Keys.own, "This phone"), device(Keys.phone, "Old phone")]), ("beta", [device(Keys.phone, "Old phone")])]))
        let remover = ScriptedRemover()
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        await store.remove(target)
        #expect(remover.plans.isEmpty)
        #expect(store.removalFailures[target.id]?.contains("betaでのただ 1 つの端末です") == true)
    }

    @Test("this device's own removal that took effect says so, for the shell to make a new key; held by the root's node it does not")
    func thisDevice() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        let store = store(source, remover)
        await store.refresh()
        await store.remove(try row(store, Keys.own))
        #expect(store.didRemoveThisDevice)
        let held = ScriptedRemover()
        held.set(.success([DeviceChangeResult(outcome: .pending, message: nil)]))
        let other = self.store(source, held)
        await other.refresh()
        await other.remove(try row(other, Keys.own))
        #expect(!other.didRemoveThisDevice)
    }

    @Test("a removal under way is not started again while it is, and it says so")
    func inFlight() async throws {
        let source = ListSource(list([("acme", both)]))
        let remover = ScriptedRemover()
        remover.hold()
        let store = store(source, remover)
        await store.refresh()
        let target = try row(store, Keys.phone)
        let first = Task { await store.remove(target) }
        while !remover.isHolding { await Task.yield() }
        #expect(store.removing == [target.id])
        await store.remove(target)
        #expect(remover.plans.count == 1)
        remover.release()
        await first.value
        #expect(store.removing.isEmpty)
    }
}

private extension Array {
    func asyncMap<T>(_ transform: (Element) async throws -> T) async rethrows -> [T] {
        var result: [T] = []
        for element in self { result.append(try await transform(element)) }
        return result
    }
}
