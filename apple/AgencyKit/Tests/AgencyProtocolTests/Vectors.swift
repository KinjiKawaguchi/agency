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

    let accountPublicKey: Data
    let devicePublicKey: Data
    let devicePairingCode: String
    let fingerprint: String
    let envelope: Signed
    let deviceChange: Signed
    let org: Signed

    static let shared = try! Vectors.load()

    private static func load() throws -> Vectors {
        // Tests/AgencyProtocolTests/Vectors.swift: five levels up is the repository root.
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/vectors.json"))
        let root = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        func object(_ key: String) -> [String: String] { root[key] as! [String: String] }
        func signed(_ key: String) -> Signed {
            let fields = object(key)
            return Signed(
                canonical: fields["canonical"]!,
                signature: Data(base64Encoded: fields["signature"]!)!,
                signer: fields["signer"] ?? ""
            )
        }
        let device = object("device")
        return Vectors(
            accountPublicKey: Data(base64Encoded: root["publicKey"] as! String)!,
            devicePublicKey: Data(base64Encoded: device["publicKey"]!)!,
            devicePairingCode: device["pairingCode"]!,
            fingerprint: root["fingerprint"] as! String,
            envelope: signed("envelope"),
            deviceChange: signed("deviceChange"),
            org: signed("org")
        )
    }

    /// Whether `key` made `signature` over the text of `kind` and `canonical`.
    static func verifies(_ key: Data, signature: Data, kind: String, canonical: String) -> Bool {
        guard let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: key) else { return false }
        return publicKey.isValidSignature(signature, for: Data("agency/v1/\(kind)\n\(canonical)".utf8))
    }
}
