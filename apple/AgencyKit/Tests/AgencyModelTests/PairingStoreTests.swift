@testable import AgencyAPI
import AgencyProtocol
@testable import AgencyModel
import Foundation
import Testing

private final class FakeClient: PairingClient, @unchecked Sendable {
    private let lock = NSLock()
    var list: Result<DeviceList, Error>
    var requestAnswer: Result<DeviceRequest, Error>
    private var _requests: [(label: String, key: String)] = []
    private var _lists = 0
    var gate: CheckedContinuation<Void, Never>?
    var holdsRequest = false

    init(list: Result<DeviceList, Error>, request: Result<DeviceRequest, Error> = .success(Fixture.decode(DeviceRequest.self, "device-request"))) {
        self.list = list
        requestAnswer = request
    }

    var requests: [(label: String, key: String)] { lock.withLock { _requests } }
    var lists: Int { lock.withLock { _lists } }

    func devices() async throws -> DeviceList { try lock.withLock { _lists += 1; return try list.get() } }
    func requestDevice(label: String, publicKey: String) async throws -> DeviceRequest {
        lock.withLock { _requests.append((label, publicKey)) }
        if lock.withLock({ holdsRequest }) {
            await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        return try requestAnswer.get()
    }
    var isHolding: Bool { lock.withLock { gate != nil } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }
}

@MainActor
@Suite("PairingStore")
struct PairingStoreTests {
    private let key = try! SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32)).publicKey

    private func list(devices: [(org: String, key: String)] = [], requests: [String] = []) -> Result<DeviceList, Error> {
        let orgs = Dictionary(grouping: devices, by: \.org).map { org, items in
            #"{"org":"\#(org)","devices":[\#(items.map { #"{"addedAt":"2026-11-24T12:00:00Z","id":"d-x","label":"x","publicKey":"\#($0.key)"}"# }.joined(separator: ","))]}"#
        }
        let reqs = requests.map { #"{"deviceId":"d-r","label":"iPhone","pairingCode":"a652-124a-fda7","publicKey":"\#($0)","requestedAt":"2026-11-24T12:00:00Z"}"# }
        let json = #"{"orgs":[\#(orgs.joined(separator: ","))],"requests":[\#(reqs.joined(separator: ","))]}"#
        return .success(try! JSONDecoder().decode(DeviceList.self, from: Data(json.utf8)))
    }

    private func store(_ client: FakeClient) -> PairingStore {
        PairingStore(client: client, key: key, label: "iPhone")
    }

    @Test("the code shown is this key's, computed as the node does")
    func code() {
        #expect(store(FakeClient(list: list())).pairingCode == "a652-124a-fda7")
    }

    @Test("a key that is a device in an org is ready, with the orgs it is added to")
    func ready() async {
        let client = FakeClient(list: list(devices: [("acme", key.string), ("beta", key.string), ("gamma", "someone-else")]))
        let store = store(client)
        #expect(store.status == .checking)
        await store.refresh()
        #expect(store.isReady)
        if case .ready(let orgs) = store.status { #expect(Set(orgs) == ["acme", "beta"]) } else { Issue.record("should be ready") }
    }

    @Test("a request with this key waits, and shows the node's code")
    func waiting() async {
        let client = FakeClient(list: list(devices: [("acme", "other")], requests: ["p256:other", key.string]))
        let store = store(client)
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .fromDevices(labels: ["x"])))
        #expect(!store.isReady)
    }

    private func listWithDevices(_ devices: [(org: String, id: String, label: String, at: String)], requests: [String] = []) -> Result<DeviceList, Error> {
        let orgs = Dictionary(grouping: devices, by: \.org).map { org, items in
            #"{"org":"\#(org)","devices":[\#(items.map { #"{"addedAt":"\#($0.at)","id":"\#($0.id)","label":"\#($0.label)","publicKey":"k-\#($0.id)"}"# }.joined(separator: ","))]}"#
        }
        let reqs = requests.map { #"{"deviceId":"d-r","label":"iPhone","pairingCode":"a652-124a-fda7","publicKey":"\#($0)","requestedAt":"2026-11-24T12:00:00Z"}"# }
        return .success(try! JSONDecoder().decode(DeviceList.self, from: Data(#"{"orgs":[\#(orgs.joined(separator: ","))],"requests":[\#(reqs.joined(separator: ","))]}"#.utf8)))
    }

    @Test("with devices already, approval is from them, named by their labels, oldest first, each once")
    func approvalFromDevices() async {
        let client = FakeClient(list: listWithDevices([
            ("acme", "d-2", "iPad", "2026-11-25T12:00:00Z"), ("acme", "d-1", "MacBook", "2026-11-24T12:00:00Z"),
            ("beta", "d-1", "MacBook", "2026-11-24T12:00:00Z"),
        ], requests: [key.string]))
        let store = store(client)
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .fromDevices(labels: ["MacBook", "iPad"])))
    }

    @Test("with no device in any org, approval is at the node's machine with the command")
    func approvalAtNode() async {
        let client = FakeClient(list: list(devices: [("acme", key.string)].filter { _ in false }, requests: [key.string]))
        let store = store(client)
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .atTheNode(command: PairingStore.defaultCommand)))
        #expect(PairingStore.defaultCommand == "agency device-add")
    }

    private func listWithNode(command: String?, requests: [String]) -> Result<DeviceList, Error> {
        let node = command.map { #","node":{"url":"https://agency.example","dataDir":"/srv/agency","command":\#(String(decoding: try! JSONEncoder().encode($0), as: UTF8.self))}"# } ?? ""
        let reqs = requests.map { #"{"deviceId":"d-r","label":"iPhone","pairingCode":"a652-124a-fda7","publicKey":"\#($0)","requestedAt":"2026-11-24T12:00:00Z"}"# }
        return .success(try! JSONDecoder().decode(DeviceList.self, from: Data(#"{"orgs":[],"requests":[\#(reqs.joined(separator: ","))]\#(node)}"#.utf8)))
    }

    @Test("the node's own command is shown when it gave one, and the default when it did not")
    func nodeCommand() async {
        let given = store(FakeClient(list: listWithNode(command: "/opt/agency/bin/agency --dir /srv/agency device-add", requests: [key.string])))
        await given.refresh()
        #expect(given.status == .waiting(code: "a652-124a-fda7", approval: .atTheNode(command: "/opt/agency/bin/agency --dir /srv/agency device-add")))
        let absent = store(FakeClient(list: listWithNode(command: nil, requests: [key.string])))
        await absent.refresh()
        #expect(absent.status == .waiting(code: "a652-124a-fda7", approval: .atTheNode(command: "agency device-add")))
    }

    @Test("a command that is empty, too long, or holds a control character is ignored", arguments: [
        "", "   ", String(repeating: "a", count: 501), "agency device-add\u{07}", "agency\ndevice-add", "agency\u{1B}[2Jdevice-add",
    ])
    func badCommand(command: String) async {
        let store = store(FakeClient(list: listWithNode(command: command, requests: [key.string])))
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .atTheNode(command: "agency device-add")))
    }

    @Test("a command of exactly 500 characters is shown")
    func longestCommand() async {
        let command = String(repeating: "a", count: 500)
        let store = store(FakeClient(list: listWithNode(command: command, requests: [key.string])))
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .atTheNode(command: command)))
    }

    @Test("when the account has devices the node's command is not needed")
    func devicesBeatCommand() async {
        let json = #"{"orgs":[{"org":"acme","devices":[{"addedAt":"2026-11-24T12:00:00Z","id":"d-1","label":"MacBook","publicKey":"k1"}]}],"requests":[{"deviceId":"d-r","label":"iPhone","pairingCode":"a652-124a-fda7","publicKey":"\#(key.string)","requestedAt":"2026-11-24T12:00:00Z"}],"node":{"command":"agency device-add"}}"#
        let store = store(FakeClient(list: .success(try! JSONDecoder().decode(DeviceList.self, from: Data(json.utf8)))))
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .fromDevices(labels: ["MacBook"])))
    }

    @Test("asking when the account has devices says to approve from them")
    func askingWithDevices() async {
        let client = FakeClient(list: listWithDevices([("acme", "d-1", "MacBook", "2026-11-24T12:00:00Z")]))
        let store = store(client)
        await store.requestAdd()
        #expect(store.status == .waiting(code: "506e-f187-9d74", approval: .fromDevices(labels: ["MacBook"])))
    }

    @Test("when the list cannot be read right after asking, the node's machine is the way that always works")
    func askingWithoutTheList() async {
        let client = FakeClient(list: .failure(NodeError.http(status: 503, message: nil)))
        let store = store(client)
        await store.requestAdd()
        #expect(store.status == .waiting(code: "506e-f187-9d74", approval: .atTheNode(command: PairingStore.defaultCommand)))
    }

    @Test("a key nobody has seen is not requested yet")
    func notRequested() async {
        let store = store(FakeClient(list: list(devices: [("acme", "other")], requests: ["p256:other"])))
        await store.refresh()
        #expect(store.status == .notRequested)
    }

    @Test("a device of another key in the org does not make this one ready")
    func otherKey() async {
        let store = store(FakeClient(list: list(devices: [("acme", "someone-else")])))
        await store.refresh()
        #expect(!store.isReady)
    }

    @Test("a failed read says why")
    func failed() async {
        let store = store(FakeClient(list: .failure(NodeError.http(status: 502, message: nil))))
        await store.refresh()
        #expect(store.status == .failed("HTTP 502"))
    }

    @Test("asking sends the label and this key and shows the code the node answers with")
    func asking() async {
        let client = FakeClient(list: list())
        let store = store(client)
        await store.refresh()
        await store.requestAdd()
        #expect(client.requests.count == 1)
        #expect(client.requests[0].label == "iPhone")
        #expect(client.requests[0].key == key.string)
        #expect(store.status == .waiting(code: "506e-f187-9d74", approval: .atTheNode(command: "agency device-add")))
        #expect(!store.isAsking)
    }

    @Test("a refused request says why and can be tried again")
    func askFails() async {
        let client = FakeClient(list: list(), request: .failure(NodeError.http(status: 400, message: "label is too long")))
        let store = store(client)
        await store.requestAdd()
        #expect(store.status == .failed("label is too long"))
        client.requestAnswer = .success(Fixture.decode(DeviceRequest.self, "device-request"))
        await store.requestAdd()
        #expect(store.status == .waiting(code: "506e-f187-9d74", approval: .atTheNode(command: "agency device-add")))
    }

    @Test("while a request is out, asking again sends nothing more")
    func noDoubleAsk() async {
        let client = FakeClient(list: list())
        client.holdsRequest = true
        let store = store(client)
        let first = Task { await store.requestAdd() }
        while !client.isHolding { await Task.yield() }
        #expect(store.isAsking)
        await store.requestAdd()
        client.release()
        await first.value
        #expect(client.requests.count == 1)
    }

    @Test("after the person adds it, a check turns waiting into ready")
    func becomesReady() async {
        let client = FakeClient(list: list(requests: [key.string]))
        let store = store(client)
        await store.refresh()
        #expect(store.status == .waiting(code: "a652-124a-fda7", approval: .atTheNode(command: "agency device-add")))
        client.list = list(devices: [("acme", key.string)])
        await store.refresh()
        #expect(store.isReady)
    }
}
