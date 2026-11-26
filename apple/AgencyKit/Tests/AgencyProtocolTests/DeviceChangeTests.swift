import Foundation
import Testing
@testable import AgencyProtocol

@Suite("DeviceChange")
struct DeviceChangeTests {
    private let key = Vectors.shared.p256.publicKey
    private let at = "2026-11-24T12:00:00Z"

    private func add(id: String = "d-a652124afda7", label: String = "iPhone", key: String? = nil, org: String = "acme", account: String = "alice") -> DeviceChange {
        DeviceChange(
            org: org, account: account,
            operation: .add(.init(id: id, label: label, publicKey: key ?? self.key, addedAt: at)), at: at
        )
    }

    @Test("an add has the whole device inside, in the canonical order of the protocol's example")
    func addBody() throws {
        let change = add()
        #expect(try CanonicalJSON.encode(change.json)
            == #"{"account":"alice","at":"2026-11-24T12:00:00Z","device":{"addedAt":"2026-11-24T12:00:00Z","id":"d-a652124afda7","label":"iPhone","publicKey":"\#(key)"},"op":"add","org":"acme"}"#)
    }

    @Test("a removal names the device by its id alone")
    func removeBody() throws {
        let change = DeviceChange(org: "acme", account: "alice", operation: .remove("phone"), at: at)
        #expect(try CanonicalJSON.encode(change.json) == #"{"account":"alice","at":"2026-11-24T12:00:00Z","device":"phone","op":"remove","org":"acme"}"#)
    }

    @Test("the signed text of a device change has the device kind")
    func signedTextKind() throws {
        let text = try signedText(kind: .device, body: add().json)
        #expect(String(decoding: text, as: UTF8.self).hasPrefix("agency/v1/device\n{"))
    }

    @Test("the device change of the vectors encodes to the vector (an Ed25519 device added by the account)")
    func vectorChange() throws {
        let vectors = Vectors.shared
        let change = DeviceChange(
            org: "acme", account: "alice",
            operation: .add(.init(id: "laptop", label: "Laptop", publicKey: vectors.devicePublicKey.base64EncodedString(), addedAt: at)), at: at
        )
        try change.validate()
        #expect(try CanonicalJSON.encode(change.json) == vectors.deviceChange.canonical)
    }

    @Test("a good add and a good removal are valid")
    func valid() throws {
        try add().validate()
        try DeviceChange(org: "acme", account: "alice", operation: .remove("d-a652124afda7"), at: at).validate()
        try add(label: String(repeating: "端", count: 80)).validate()
    }

    @Test("ids, label, key and time have the node's shape")
    func invalid() {
        #expect(throws: DeviceChangeError.malformedID(field: "org")) { try add(org: "Acme").validate() }
        #expect(throws: DeviceChangeError.malformedID(field: "account")) { try add(account: "").validate() }
        #expect(throws: DeviceChangeError.malformedID(field: "device")) { try add(id: "D-1").validate() }
        #expect(throws: DeviceChangeError.malformedID(field: "device")) { try DeviceChange(org: "acme", account: "alice", operation: .remove("a b"), at: at).validate() }
        #expect(throws: DeviceChangeError.malformedLabel) { try add(label: "").validate() }
        #expect(throws: DeviceChangeError.malformedLabel) { try add(label: " iPhone").validate() }
        #expect(throws: DeviceChangeError.malformedLabel) { try add(label: String(repeating: "a", count: 81)).validate() }
        #expect(throws: DeviceChangeError.malformedKey) { try add(key: "p256:AAAA").validate() }
        #expect(throws: DeviceChangeError.malformedTime) {
            try DeviceChange(org: "acme", account: "alice", operation: .remove("phone"), at: "yesterday").validate()
        }
        #expect(throws: DeviceChangeError.malformedTime) {
            try DeviceChange(org: "acme", account: "alice", operation: .add(.init(id: "phone", label: "x", publicKey: key, addedAt: "soon")), at: at).validate()
        }
    }

    // MARK: The node's vectors, signed by the P-256 device

    private var vectors: Vectors { Vectors.shared }

    private func member(_ value: JSONValue?, _ key: String) -> JSONValue? { value?[key] }

    /// The change in a vector's canonical text, built the way the app builds one.
    private func change(from body: JSONValue) throws -> DeviceChange {
        let org = try #require(body["org"]?.text), account = try #require(body["account"]?.text), at = try #require(body["at"]?.text)
        switch try #require(body["op"]?.text) {
        case "add":
            let device = try #require(body["device"])
            return DeviceChange(
                org: org, account: account,
                operation: .add(.init(
                    id: try #require(device["id"]?.text), label: try #require(device["label"]?.text),
                    publicKey: try #require(device["publicKey"]?.text), addedAt: try #require(device["addedAt"]?.text)
                )), at: at
            )
        default:
            return DeviceChange(org: org, account: account, operation: .remove(try #require(body["device"]?.text)), at: at)
        }
    }

    @Test("an add of a P-256 device encodes to the node's canonical text, and its signature verifies over our signed text")
    func addVector() throws {
        let vector = vectors.deviceChangeAdd
        let built = try change(from: vector.body)
        try built.validate()
        #expect(try CanonicalJSON.encode(built.json) == vector.canonical)
        let text = try signedText(kind: .device, body: built.json)
        #expect(text == Data(vector.signedText.utf8))
        #expect(vector.signedText.hasPrefix("agency/v1/device\n"))
        let key = try DevicePublicKey(string: vectors.deviceChangeSignerKey)
        #expect(key.verifies(vector.signature, over: text))
        #expect(key.fingerprint.deviceID == vector.signer)
        guard case .add(let device) = built.operation else { Issue.record("should be an add"); return }
        #expect((try? DevicePublicKey(string: device.publicKey)) != nil)
    }

    @Test("a removal by bare id encodes to the node's canonical text, and its signature verifies")
    func removeVector() throws {
        let vector = vectors.deviceChangeRemove
        let built = try change(from: vector.body)
        try built.validate()
        guard case .remove = built.operation else { Issue.record("should be a removal"); return }
        #expect(try CanonicalJSON.encode(built.json) == vector.canonical)
        let text = try signedText(kind: .device, body: built.json)
        #expect(text == Data(vector.signedText.utf8))
        #expect(try DevicePublicKey(string: vectors.deviceChangeSignerKey).verifies(vector.signature, over: text))
    }

    @Test("a signature over one change does not verify for the other, for the envelope kind, or for a changed time")
    func notInterchangeable() throws {
        let key = try DevicePublicKey(string: vectors.deviceChangeSignerKey)
        let add = vectors.deviceChangeAdd, remove = vectors.deviceChangeRemove
        #expect(!key.verifies(add.signature, over: Data(remove.signedText.utf8)))
        #expect(!key.verifies(remove.signature, over: Data(add.signedText.utf8)))
        let asEnvelope = Data(add.signedText.replacingOccurrences(of: "agency/v1/device\n", with: "agency/v1/envelope\n").utf8)
        #expect(!key.verifies(add.signature, over: asEnvelope))
        let later = Data(add.signedText.replacingOccurrences(of: "2026-11-24T12:00:00Z", with: "2026-11-24T12:00:01Z").utf8)
        #expect(!key.verifies(add.signature, over: later))
    }

    @Test("a change made here, signed by the same key, verifies with the key and in the node's form")
    func ownSignature() async throws {
        let signer = try SoftwareP256Signer(rawPrivateKey: Data(repeating: 0x0b, count: 32))
        #expect(signer.publicKey.string == vectors.deviceChangeSignerKey)
        let built = try change(from: vectors.deviceChangeAdd.body)
        let text = try signedText(kind: .device, body: built.json)
        let signature = try await signer.sign(text, reason: "test")
        #expect(signer.publicKey.verifies(signature, over: text))
    }
}
