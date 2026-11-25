import AgencyProtocol
import Crypto
import Foundation

/// `testdata/vectors.json`: keys from fixed seeds, and the canonical JSON and signatures of fixed
/// bodies, made by the Rust implementation.
struct Vectors {
    struct Signed {
        let canonical: String
        let signature: Data
        let signer: String
    }

    /// A value and the exact canonical text it must encode to.
    struct CanonicalCase {
        let name: String
        let value: JSONValue
        let canonical: String
    }

    let accountPublicKey: Data
    let devicePublicKey: Data
    let devicePairingCode: String
    let fingerprint: String
    let envelope: Signed
    let commentEnvelope: Signed
    let deviceChange: Signed
    let org: Signed
    let canonicalCases: [CanonicalCase]

    static let shared = try! Vectors.load()

    static func fileURL(_ path: String, from file: String = #filePath) -> URL {
        // Tests/AgencyProtocolTests/<file>.swift: five levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return url.appendingPathComponent(path)
    }

    private static func load() throws -> Vectors {
        let root = try JSONParser.parse(Data(contentsOf: fileURL("testdata/vectors.json")))
        func string(_ value: JSONValue?) -> String { value!.text! }
        func signed(_ key: String) -> Signed {
            let fields = root[key]!
            return Signed(
                canonical: string(fields["canonical"]),
                signature: Data(base64Encoded: string(fields["signature"]))!,
                signer: fields["signer"]?.text ?? ""
            )
        }
        let device = root["device"]!
        let cases = root["canonical"]!.items.map {
            CanonicalCase(name: string($0["name"]), value: $0["value"]!, canonical: string($0["canonical"]))
        }
        return Vectors(
            accountPublicKey: Data(base64Encoded: string(root["publicKey"]))!,
            devicePublicKey: Data(base64Encoded: string(device["publicKey"]))!,
            devicePairingCode: string(device["pairingCode"]),
            fingerprint: string(root["fingerprint"]),
            envelope: signed("envelope"),
            commentEnvelope: signed("commentEnvelope"),
            deviceChange: signed("deviceChange"),
            org: signed("org"),
            canonicalCases: cases
        )
    }

    /// Whether `key` made `signature` over the text of `kind` and `canonical`.
    static func verifies(_ key: Data, signature: Data, kind: String, canonical: String) -> Bool {
        guard let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: key) else { return false }
        return publicKey.isValidSignature(signature, for: Data("agency/v1/\(kind)\n\(canonical)".utf8))
    }
}
