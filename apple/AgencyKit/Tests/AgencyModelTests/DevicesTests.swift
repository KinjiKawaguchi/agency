@testable import AgencyAPI
import AgencyProtocol
@testable import AgencyModel
import Foundation
import Testing

private func list(_ json: String) -> DeviceList { try! JSONDecoder().decode(DeviceList.self, from: Data(json.utf8)) }

private func device(_ id: String, _ key: String, label: String = "x", at: String = "2026-11-24T12:00:00Z") -> String {
    #"{"addedAt":"\#(at)","id":"\#(id)","label":"\#(label)","publicKey":"\#(key)"}"#
}

private func request(_ id: String, _ code: String, _ key: String, label: String = "iPhone") -> String {
    #"{"deviceId":"\#(id)","label":"\#(label)","pairingCode":"\#(code)","publicKey":"\#(key)","requestedAt":"2026-11-24T12:00:00Z"}"#
}

@Suite("DeviceFacts")
struct DeviceFactsTests {
    @Test("the id a key has in an org, only where it was added")
    func idInOrg() {
        let devices = list(#"{"orgs":[{"org":"acme","devices":[\#(device("d-1", "K1"))]},{"org":"beta","devices":[\#(device("d-2", "K2"))]}],"requests":[]}"#)
        #expect(DeviceFacts.deviceID(in: devices, key: "K1", org: "acme") == "d-1")
        #expect(DeviceFacts.deviceID(in: devices, key: "K1", org: "beta") == nil)
        #expect(DeviceFacts.deviceID(in: devices, key: "K1", org: "nope") == nil)
    }

    @Test("the orgs that lack a key")
    func lacking() {
        let devices = list(#"{"orgs":[{"org":"acme","devices":[\#(device("d-1", "K1"))]},{"org":"beta","devices":[]}],"requests":[]}"#)
        #expect(DeviceFacts.orgsLacking(devices, key: "K1") == ["beta"])
        #expect(DeviceFacts.orgsLacking(devices, key: "K9") == ["acme", "beta"])
    }

    @Test("requests of other devices leave out this phone's own")
    func fromOthers() {
        let devices = list(#"{"orgs":[],"requests":[\#(request("d-a", "a", "K1")),\#(request("d-b", "b", "K2"))]}"#)
        #expect(DeviceFacts.requestsFromOthers(devices, ownKey: "K1").map(\.deviceId) == ["d-b"])
    }

    @Test("one row for a device in several orgs, the newest first, this phone marked")
    func rows() {
        let devices = list("""
        {"orgs":[{"org":"acme","devices":[\(device("d-1", "K1", label: "Laptop", at: "2026-11-23T12:00:00Z")),\(device("d-2", "K2", label: "iPhone", at: "2026-11-24T12:00:00Z"))]},
        {"org":"beta","devices":[\(device("d-2", "K2", label: "iPhone", at: "2026-11-24T12:00:00Z"))]}],"requests":[]}
        """)
        let rows = DeviceFacts.rows(devices, ownKey: "K2")
        #expect(rows.map(\.id) == ["d-2", "d-1"])
        #expect(rows[0].orgs == ["acme", "beta"])
        #expect(rows[0].isThisPhone && !rows[1].isThisPhone)
        #expect(rows[1].label == "Laptop")
    }

    @Test("the node's real list reads")
    func fixture() {
        let devices = Fixture.decode(DeviceList.self, "device-list")
        #expect(DeviceFacts.rows(devices, ownKey: "other").map(\.id) == ["d-dbc298251c51"])
        #expect(DeviceFacts.requestsFromOthers(devices, ownKey: "other").count == 1)
    }
}

private final class FakePoster: DeviceChangePoster, @unchecked Sendable {
    private let lock = NSLock()
    var devices: DeviceList
    var result: Result<DeviceChangeResult, Error> = .success(DeviceChangeResult(outcome: .applied, message: nil))
    /// What the next submissions answer, one each, before `result` is used.
    var queue: [Result<DeviceChangeResult, Error>] = []
    private var _submitted: [SignedDeviceChange] = []
    private var _calls: [String] = []
    private var times = 0

    init(devices: DeviceList) { self.devices = devices }

    var submitted: [SignedDeviceChange] { lock.withLock { _submitted } }
    var calls: [String] { lock.withLock { _calls } }

    func me() async throws -> Me { Fixture.decode(Me.self, "me") }
    func devices() async throws -> DeviceList { lock.withLock { _calls.append("devices") }; return devices }
    func refreshSession() async throws -> Session {
        lock.withLock { _calls.append("session") }
        return try JSONDecoder().decode(Session.self, from: Data(#"{"writeToken":"w","now":"2026-11-24T12:00:00Z"}"#.utf8))
    }
    func nodeNow() async -> Date {
        lock.withLock { times += 1; return Date(timeIntervalSince1970: 1_795_521_600 + Double(times)) }
    }
    func submitDeviceChange(_ change: SignedDeviceChange) async throws -> DeviceChangeResult {
        let next = lock.withLock { () -> Result<DeviceChangeResult, Error> in
            _submitted.append(change)
            return queue.isEmpty ? result : queue.removeFirst()
        }
        return try next.get()
    }
}

private final class RecordingSigner: DeviceSigner, @unchecked Sendable {
    let inner = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32))
    private let lock = NSLock()
    private var _reasons: [String] = []
    var publicKey: DevicePublicKey { inner.publicKey }
    var reasons: [String] { lock.withLock { _reasons } }
    func sign(_ text: Data, reason: String) async throws -> Data {
        lock.withLock { _reasons.append(reason) }
        return try await inner.sign(text, reason: reason)
    }
}

@Suite("DeviceChangeSender")
struct DeviceChangeSenderTests {
    private let signer = RecordingSigner()
    // A real request: the key, and the id and code its fingerprint makes.
    private let newKey = Fixture.decode(DeviceRequest.self, "device-request")

    private var own: String { signer.publicKey.string }

    private func devices(_ orgs: String) -> DeviceList { list(#"{"orgs":[\#(orgs)],"requests":[]}"#) }

    private func sender(_ poster: FakePoster) -> DeviceChangeSender { DeviceChangeSender(poster: poster, signer: signer) }

    @Test("the fixture's request is a real one: its id and code are the key's")
    func fixtureIsReal() throws {
        let key = try DevicePublicKey(string: newKey.publicKey)
        #expect(newKey.deviceId == key.fingerprint.deviceID)
        #expect(newKey.pairingCode == key.fingerprint.pairingCode)
    }

    @Test("what is signed is the label the node gave, never the shown one: the request is added with its label as it was written, and the person is asked about the shown one")
    func signsTheRawLabel() async throws {
        let raw = "iPhone\u{202E} of Erin"
        var object = try #require(JSONSerialization.jsonObject(with: Fixture.data("device-request")) as? [String: Any])
        object["label"] = raw
        let request = try JSONDecoder().decode(DeviceRequest.self, from: JSONSerialization.data(withJSONObject: object))
        #expect(request.label == "iPhone? of Erin" && request.rawLabel == raw)
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-a652124afda7", own, label: "iPhone"))]}"#))
        _ = try await sender(poster).add(request)
        guard case .add(let added) = try #require(poster.submitted.first).change.operation else { Issue.record("should be an add"); return }
        #expect(added.label == raw, "the signature covers the bytes the node wrote")
        #expect(signer.reasons == ["端末を追加する: iPhone? of Erin"], "what the person reads is the shown label")
    }

    @Test("an add is signed by this device as its id in the org, and the signature verifies over the device text")
    func adds() async throws {
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-a652124afda7", own, label: "iPhone"))]}"#))
        let results = try await sender(poster).add(newKey)
        #expect(results == [DeviceChangeResult(outcome: .applied, message: nil)])
        let sent = try #require(poster.submitted.first)
        #expect(sent.signer == "d-a652124afda7")
        #expect(sent.change.org == "acme" && sent.change.account == "alice")
        guard case .add(let added) = sent.change.operation else { Issue.record("should be an add"); return }
        #expect(added.id == newKey.deviceId && added.label == "iPhone" && added.publicKey == newKey.publicKey)
        let text = try signedText(kind: .device, body: sent.change.json)
        #expect(signer.publicKey.verifies(try #require(Data(base64Encoded: sent.signature)), over: text))
        #expect(signer.reasons == ["端末を追加する: iPhone"])
        #expect(poster.calls.first == "session")
    }

    @Test("each org that lacks the new device and holds this one gets its own signed change")
    func severalOrgs() async throws {
        let poster = FakePoster(devices: devices("""
        {"org":"acme","devices":[\(device("d-own-acme", own))]},
        {"org":"beta","devices":[\(device("d-own-beta", own)),\(device(newKey.deviceId, newKey.publicKey))]},
        {"org":"gamma","devices":[\(device("d-own-gamma", own))]},
        {"org":"delta","devices":[]}
        """))
        let results = try await sender(poster).add(newKey)
        #expect(results.count == 2)
        #expect(poster.submitted.map(\.change.org) == ["acme", "gamma"])
        #expect(poster.submitted.map(\.signer) == ["d-own-acme", "d-own-gamma"])
        #expect(signer.reasons.count == 2)
        // Each change has its own time from the node.
        #expect(Set(poster.submitted.map(\.change.at)).count == 2)
    }

    @Test("with no org to add it to, nothing is signed")
    func nowhere() async throws {
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-x", "someone-else"))]}"#))
        await #expect(throws: DeviceChangeSenderError.nowhereToAdd) { try await sender(poster).add(newKey) }
        #expect(signer.reasons.isEmpty && poster.submitted.isEmpty)
        let already = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-own", own)),\#(device(newKey.deviceId, newKey.publicKey))]}"#))
        await #expect(throws: DeviceChangeSenderError.nowhereToAdd) { try await sender(already).add(newKey) }
    }

    @Test("a code the node showed that is not the key's is refused before anyone is asked")
    func codeMismatch() async throws {
        let lying = try JSONDecoder().decode(DeviceRequest.self, from: Data(request(newKey.deviceId, "0000-0000-0000", newKey.publicKey).utf8))
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-own", own))]}"#))
        await #expect(throws: DeviceChangeSenderError.codeMismatch) { try await sender(poster).add(lying) }
        #expect(signer.reasons.isEmpty && poster.submitted.isEmpty && poster.calls.isEmpty)
    }

    @Test("an id that is not the key's, or a key that is not a key, is refused")
    func malformed() async throws {
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-own", own))]}"#))
        let wrongID = try JSONDecoder().decode(DeviceRequest.self, from: Data(request("d-000000000000", newKey.pairingCode, newKey.publicKey).utf8))
        let badKey = try JSONDecoder().decode(DeviceRequest.self, from: Data(request(newKey.deviceId, newKey.pairingCode, "p256:AAAA").utf8))
        await #expect(throws: DeviceChangeSenderError.malformedRequest) { try await sender(poster).add(wrongID) }
        await #expect(throws: DeviceChangeSenderError.malformedRequest) { try await sender(poster).add(badKey) }
        #expect(signer.reasons.isEmpty)
    }

    @Test("a label the node would refuse is refused before Face ID")
    func invalidLabel() async throws {
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-own", own))]}"#))
        let long = try JSONDecoder().decode(DeviceRequest.self, from: Data(request(newKey.deviceId, newKey.pairingCode, newKey.publicKey, label: String(repeating: "a", count: 81)).utf8))
        await #expect(throws: DeviceChangeError.malformedLabel) { try await sender(poster).add(long) }
        #expect(signer.reasons.isEmpty && poster.submitted.isEmpty)
    }

    @Test("a refusal by the node is passed on")
    func refused() async throws {
        let poster = FakePoster(devices: devices(#"{"org":"acme","devices":[\#(device("d-own", own))]}"#))
        poster.result = .failure(NodeError.http(status: 409, message: "refused"))
        await #expect(throws: NodeError.http(status: 409, message: "refused")) { try await sender(poster).add(newKey) }
    }

    private func twoOrgs() -> DeviceList {
        devices(#"{"org":"acme","devices":[\#(device("d-a652124afda7", own, label: "iPhone"))]},{"org":"beta","devices":[\#(device("d-b00000000001", own, label: "iPhone"))]}"#)
    }

    @Test("a change the root's node holds is answered as held, and the next org is still asked")
    func held() async throws {
        let poster = FakePoster(devices: twoOrgs())
        poster.queue = [.success(DeviceChangeResult(outcome: .pending, message: "Approved here"))]
        let results = try await sender(poster).add(newKey)
        #expect(results.map(\.outcome) == [.pending, .applied])
        #expect(poster.submitted.count == 2)
    }

    @Test("a later org's failure keeps what the node answered for the orgs before it; the first org's failure is the failure itself")
    func partly() async throws {
        let poster = FakePoster(devices: twoOrgs())
        poster.queue = [.success(DeviceChangeResult(outcome: .pending, message: nil)), .failure(NodeError.http(status: 403, message: "no"))]
        do {
            _ = try await sender(poster).add(newKey)
            Issue.record("should fail")
        } catch let partly as DeviceChangePartlyAnswered {
            #expect(partly.answered.map(\.outcome) == [.pending])
            #expect(partly.cause as? NodeError == NodeError.http(status: 403, message: "no"))
        }
        let first = FakePoster(devices: twoOrgs())
        first.queue = [.failure(NodeError.http(status: 403, message: "no"))]
        await #expect(throws: NodeError.http(status: 403, message: "no")) { _ = try await sender(first).add(newKey) }
    }

    @Test("the messages are Japanese")
    func messages() {
        #expect(DeviceChangeSenderError.nowhereToAdd.errorDescription == Ja.Devices.nowhereToAdd)
        #expect(DeviceChangeSenderError.codeMismatch.errorDescription == Ja.Devices.codeMismatch)
        #expect(DeviceChangeSenderError.malformedRequest.errorDescription == Ja.Devices.malformedRequest)
    }
}

private final class FakeSource: DevicesSource, @unchecked Sendable {
    private let lock = NSLock()
    var result: Result<DeviceList, Error>
    var held: Result<[RootChangeRef], Error> = .success([])
    init(_ result: Result<DeviceList, Error>) { self.result = result }
    func devices() async throws -> DeviceList { try lock.withLock { try result.get() } }
    func heldChanges() async throws -> [RootChangeRef] { try lock.withLock { try held.get() } }
}

private final class FakeAdder: DeviceAdder, @unchecked Sendable {
    private let lock = NSLock()
    var result: Result<[DeviceChangeResult], Error> = .success([DeviceChangeResult(outcome: .applied, message: nil)])
    private var _added: [DeviceRequest] = []
    var gate: CheckedContinuation<Void, Never>?
    var holds = false
    var added: [DeviceRequest] { lock.withLock { _added } }
    var isHolding: Bool { lock.withLock { gate != nil } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
    func add(_ request: DeviceRequest) async throws -> [DeviceChangeResult] {
        lock.withLock { _added.append(request) }
        if lock.withLock({ holds }) { await withCheckedContinuation { c in lock.withLock { gate = c } } }
        return try lock.withLock { try result.get() }
    }
}

@MainActor
@Suite("DevicesStore")
struct DevicesStoreTests {
    private let own = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32)).publicKey
    private let newKey = Fixture.decode(DeviceRequest.self, "device-request")

    private func store(_ source: FakeSource, _ adder: FakeAdder) -> DevicesStore {
        DevicesStore(source: source, adder: adder, ownKey: own)
    }

    private var waiting: DeviceList {
        list(#"{"orgs":[{"org":"acme","devices":[\#(device("d-own", own.string))]}],"requests":[\#(request(newKey.deviceId, newKey.pairingCode, newKey.publicKey)),\#(request("d-me", "x", own.string))]}"#)
    }

    @Test("the open screen asks again every few seconds, so a new request needs no reload")
    func polls() {
        #expect(DevicesStore.pollSeconds == 4)
    }

    @Test("it reads the devices and the requests of others")
    func refresh() async {
        let store = store(FakeSource(.success(waiting)), FakeAdder())
        #expect(!store.isLoaded)
        await store.refresh()
        #expect(store.isLoaded)
        #expect(store.rows.map(\.id) == ["d-own"])
        #expect(store.rows[0].isThisPhone)
        #expect(store.requests.map(\.deviceId) == [newKey.deviceId])
    }

    @Test("a failed read says why and keeps what was shown")
    func failure() async {
        let source = FakeSource(.success(waiting))
        let store = store(source, FakeAdder())
        await store.refresh()
        source.result = .failure(NodeError.http(status: 502, message: nil))
        await store.refresh()
        #expect(store.loadFailure == "HTTP 502")
        #expect(store.rows.count == 1)
        source.result = .success(waiting)
        await store.refresh()
        #expect(store.loadFailure == nil)
    }

    @Test("confirming adds the device, says so, and the list is read again")
    func confirm() async {
        let source = FakeSource(.success(waiting))
        let adder = FakeAdder()
        let store = store(source, adder)
        await store.refresh()
        let request = store.requests[0]
        source.result = .success(list(#"{"orgs":[{"org":"acme","devices":[\#(device("d-own", own.string)),\#(device(newKey.deviceId, newKey.publicKey))]}],"requests":[]}"#))
        await store.confirm(request)
        #expect(adder.added == [request])
        #expect(store.rows.count == 2)
        #expect(store.requests.isEmpty)
        #expect(store.confirmed.isEmpty, "a confirmed message is dropped when the request leaves the list")
        #expect(store.confirming.isEmpty)
    }

    @Test("a queued change says the org's node is asleep")
    func queued() async {
        let adder = FakeAdder()
        adder.result = .success([DeviceChangeResult(outcome: .queued, message: "node asleep")])
        let store = store(FakeSource(.success(waiting)), adder)
        await store.refresh()
        await store.confirm(store.requests[0])
        #expect(store.confirmed[newKey.publicKey] == Ja.Devices.queued("node asleep"))
    }

    @Test("a failed add says why and can be tried again")
    func addFails() async {
        let adder = FakeAdder()
        adder.result = .failure(DeviceChangeSenderError.codeMismatch)
        let store = store(FakeSource(.success(waiting)), adder)
        await store.refresh()
        await store.confirm(store.requests[0])
        #expect(store.confirmFailures[newKey.publicKey] == Ja.Devices.codeMismatch)
        adder.result = .success([DeviceChangeResult(outcome: .applied, message: nil)])
        await store.confirm(store.requests[0])
        #expect(store.confirmFailures[newKey.publicKey] == nil)
        #expect(adder.added.count == 2)
    }

    @Test("while a request is being confirmed, confirming it again adds nothing more")
    func noDouble() async {
        let adder = FakeAdder()
        adder.holds = true
        let store = store(FakeSource(.success(waiting)), adder)
        await store.refresh()
        let request = store.requests[0]
        let first = Task { await store.confirm(request) }
        while !adder.isHolding { await Task.yield() }
        #expect(store.confirming == [request.publicKey])
        await store.confirm(request)
        adder.release()
        await first.value
        #expect(adder.added.count == 1)
    }
}

@MainActor
@Suite("DevicesStore: a change the root's node holds")
struct DevicesHeldTests {
    private let own = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32)).publicKey
    private let newKey = Fixture.decode(DeviceRequest.self, "device-request")

    private var waiting: DeviceList {
        list(#"{"orgs":[{"org":"acme","devices":[\#(device("d-own", own.string))]}],"requests":[\#(request(newKey.deviceId, newKey.pairingCode, newKey.publicKey))]}"#)
    }

    private func confirmed(_ results: Result<[DeviceChangeResult], Error>) async -> DevicesStore {
        let adder = FakeAdder()
        adder.result = results
        let store = DevicesStore(source: FakeSource(.success(waiting)), adder: adder, ownKey: own)
        await store.refresh()
        await store.confirm(store.requests[0])
        return store
    }

    private func result(_ outcome: DeviceChangeResult.Outcome, _ message: String? = nil) -> DeviceChangeResult { DeviceChangeResult(outcome: outcome, message: message) }

    @Test("a pending approval is said in the app's words, with the node's message ignored, and is not said as an add")
    func pending() async {
        let store = await confirmed(.success([result(.pending, "Approved here; it takes effect once the root key's node signs it")]))
        #expect(store.held[newKey.publicKey] == "ここでは承認しました。ルート鍵のノードが署名すると有効になります。そのノードで agency root-sign を実行してください。")
        #expect(store.confirmed.isEmpty && store.confirmFailures.isEmpty)
    }

    @Test("an outcome the app does not know is never a success: said with the node's message, or without one that it is not known and where the result is")
    func unknown() async {
        let said = await confirmed(.success([result(.unknown("later"), "It will take a while.")]))
        #expect(said.held[newKey.publicKey] == "It will take a while." && said.confirmed.isEmpty)
        let silent = await confirmed(.success([result(.unknown("later"))]))
        #expect(silent.held[newKey.publicKey] == Ja.Devices.unknownOutcome && silent.confirmed.isEmpty)
        let empty = await confirmed(.success([result(.unknown("later"), "")]))
        #expect(empty.held[newKey.publicKey] == Ja.Devices.unknownOutcome && empty.confirmed.isEmpty)
    }

    @Test("with several orgs, a held one is not turned into a success by the others, and a node that was asleep is still said")
    func several() async {
        let store = await confirmed(.success([result(.pending), result(.applied), result(.pending), result(.queued, "asleep")]))
        #expect(store.held[newKey.publicKey] == RootChangeNotice.approvedHere + "\n" + Ja.Devices.queued("asleep"))
        #expect(store.confirmed.isEmpty)
    }

    @Test("an add that was answered stays an add, as before")
    func applied() async {
        let store = await confirmed(.success([result(.applied), result(.delivered)]))
        #expect(store.held.isEmpty && store.confirmed[newKey.publicKey] == Ja.Devices.confirmed("iPhone"))
    }

    @Test("a later org's failure keeps what an earlier one held, and says why")
    func partly() async {
        let store = await confirmed(.failure(DeviceChangePartlyAnswered(answered: [result(.pending)], cause: NodeError.http(status: 403, message: "no"))))
        #expect(store.held[newKey.publicKey] == RootChangeNotice.approvedHere)
        #expect(store.confirmFailures[newKey.publicKey] == "no")
    }

    private func held(_ kind: String, device: String?) -> RootChangeRef {
        RootChangeRef(id: "c1", org: "acme", what: "x", receivedAt: "2026-11-26T09:00:00Z", kind: kind, member: "alice", label: "iPhone", device: device)
    }

    @Test("a request approved before, which the node still holds, is held when the screen is opened again, and cannot be approved again")
    func heldAtTheNode() async {
        let source = FakeSource(.success(waiting))
        source.held = .success([held("addDevice", device: newKey.deviceId)])
        let store = DevicesStore(source: source, adder: FakeAdder(), ownKey: own)
        await store.refresh()
        #expect(store.held[newKey.publicKey] == RootChangeNotice.approvedHere)
        #expect(store.heldHere.isEmpty)
    }

    @Test("only the add of that device holds it: another device's, another kind's, and a held change with no device do not")
    func heldAtTheNodeIsThatDevice() async {
        let source = FakeSource(.success(waiting))
        source.held = .success([held("addDevice", device: "d-other"), held("removeDevice", device: newKey.deviceId), held("addMember", device: nil)])
        let store = DevicesStore(source: source, adder: FakeAdder(), ownKey: own)
        await store.refresh()
        #expect(store.held.isEmpty)
    }

    @Test("what the node held is not held any more once it says so, and a read of it that fails leaves what was known")
    func heldAtTheNodeGoes() async {
        let source = FakeSource(.success(waiting))
        source.held = .success([held("addDevice", device: newKey.deviceId)])
        let store = DevicesStore(source: source, adder: FakeAdder(), ownKey: own)
        await store.refresh()
        source.held = .failure(NodeError.http(status: 502, message: nil))
        await store.refresh()
        #expect(store.held[newKey.publicKey] != nil && store.loadFailure == nil)
        source.held = .success([])
        await store.refresh()
        #expect(store.held.isEmpty)
    }

    @Test("what is held goes when the request leaves the list")
    func leaves() async {
        let source = FakeSource(.success(waiting))
        let adder = FakeAdder()
        adder.result = .success([result(.pending)])
        let store = DevicesStore(source: source, adder: adder, ownKey: own)
        await store.refresh()
        await store.confirm(store.requests[0])
        #expect(!store.held.isEmpty)
        source.result = .success(list(#"{"orgs":[{"org":"acme","devices":[\#(device("d-own", own.string)),\#(device(newKey.deviceId, newKey.publicKey))]}],"requests":[]}"#))
        await store.refresh()
        #expect(store.held.isEmpty)
    }
}
