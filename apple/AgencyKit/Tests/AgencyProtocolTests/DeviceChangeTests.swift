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
}
