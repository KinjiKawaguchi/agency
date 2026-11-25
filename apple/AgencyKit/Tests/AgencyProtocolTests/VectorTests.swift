import Foundation
import Testing
@testable import AgencyProtocol

/// The Swift bytes must be the bytes the Rust side signed. Each body is built here from its parts,
/// with members out of order on purpose, and has to encode to the vector's canonical JSON.
@Suite("Protocol vectors")
struct VectorTests {
    private let vectors = Vectors.shared

    @Test("a decision envelope encodes to the vector, and its signature verifies over our bytes")
    func envelope() throws {
        let envelope = DecisionEnvelope(
            id: "00112233445566778899aabbccddeeff", kind: .answer, org: "acme", team: "app",
            target: "r-20260101-000000-abcd", text: "", choice: "ア", account: "alice",
            at: "2026-11-24T12:00:00Z"
        )
        try envelope.validate()
        #expect(try CanonicalJSON.encode(envelope.json) == vectors.envelope.canonical)
        let text = try signedText(kind: .envelope, body: envelope.json)
        #expect(text == Data("agency/v1/envelope\n\(vectors.envelope.canonical)".utf8))
        #expect(Vectors.verifies(
            vectors.devicePublicKey, signature: vectors.envelope.signature,
            kind: "envelope", canonical: try CanonicalJSON.encode(envelope.json)
        ))
    }

    @Test("a device change encodes to the vector")
    func deviceChange() throws {
        let body: JSONValue = [
            "op": "add", "org": "acme", "account": "alice", "at": "2026-11-24T12:00:00Z",
            "device": [
                "label": "Laptop", "id": "laptop", "addedAt": "2026-11-24T12:00:00Z",
                "publicKey": .string(vectors.devicePublicKey.base64EncodedString()),
            ],
        ]
        #expect(try CanonicalJSON.encode(body) == vectors.deviceChange.canonical)
        #expect(Vectors.verifies(
            vectors.accountPublicKey, signature: vectors.deviceChange.signature,
            kind: "device", canonical: try CanonicalJSON.encode(body)
        ))
    }

    @Test("an org definition, with Japanese text, an empty list and nesting, encodes to the vector")
    func org() throws {
        let key = JSONValue.string(vectors.accountPublicKey.base64EncodedString())
        let body: JSONValue = [
            "version": 1, "teams": [["node": "https://alice.example", "label": "app", "id": "app", "department": "dev"]],
            "root": ["publicKey": key, "account": "alice"], "revoked": [], "org": "acme", "label": "Acme",
            "at": "2026-11-24T12:00:00Z",
            "people": [["role": "member", "publicKey": key, "label": "Alice", "id": "owner", "department": "dev", "account": "alice"]],
            "departments": [["label": "開発", "id": "dev", "head": "owner"]],
        ]
        #expect(try CanonicalJSON.encode(body) == vectors.org.canonical)
        #expect(Vectors.verifies(
            vectors.accountPublicKey, signature: vectors.org.signature,
            kind: "org", canonical: try CanonicalJSON.encode(body)
        ))
    }

    @Test("every canonical case of the vectors encodes to the Rust side's exact text")
    func canonicalCases() throws {
        #expect(vectors.canonicalCases.count >= 8)
        for item in vectors.canonicalCases {
            #expect(try CanonicalJSON.encode(item.value) == item.canonical, "\(item.name)")
        }
    }

    @Test("a comment-only envelope in Japanese with an emoji, a newline and quotes encodes to the vector and verifies")
    func commentEnvelope() throws {
        let envelope = DecisionEnvelope(
            id: "ffeeddccbbaa99887766554433221100", kind: .answer, org: "acme", team: "app",
            target: "r-20260101-000000-abcd", text: "来週にしてください 🙏\n\"理由\"は別途", choice: nil,
            account: "alice", at: "2026-11-24T12:00:00Z"
        )
        try envelope.validate()
        let canonical = try CanonicalJSON.encode(envelope.json)
        #expect(canonical == vectors.commentEnvelope.canonical)
        #expect(!canonical.contains("choice"))
        #expect(Vectors.verifies(
            vectors.devicePublicKey, signature: vectors.commentEnvelope.signature, kind: "envelope", canonical: canonical
        ))
    }

    @Test("a signature for one kind does not verify as another")
    func kindIsPartOfTheText() {
        #expect(!Vectors.verifies(
            vectors.devicePublicKey, signature: vectors.envelope.signature,
            kind: "device", canonical: vectors.envelope.canonical
        ))
    }

    @Test("the fingerprint and pairing code are the vectors'")
    func fingerprint() {
        #expect(DeviceFingerprint(publicKey: vectors.accountPublicKey).hex == vectors.fingerprint)
        let device = DeviceFingerprint(publicKey: vectors.devicePublicKey)
        #expect(device.pairingCode == vectors.devicePairingCode)
        #expect(device.deviceID == "d-dbc298251c51")
    }
}
