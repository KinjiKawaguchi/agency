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
    let p256: P256Vector

    /// A P-256 device key made from a fixed private scalar, and the Rust side's signature over the comment envelope.
    struct P256Vector {
        let privateKey: Data
        let publicKey: String
        let fingerprint: String
        let pairingCode: String
        let deviceId: String
        let commentSignature: Data
    }

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
        let p256 = root["p256Device"]!
        let privateHex = string(p256["privateKey"])
        let privateKey = Data(stride(from: 0, to: privateHex.count, by: 2).map {
            UInt8(privateHex[privateHex.index(privateHex.startIndex, offsetBy: $0)...][..<privateHex.index(privateHex.startIndex, offsetBy: $0 + 2)], radix: 16)!
        })
        return Vectors(
            accountPublicKey: Data(base64Encoded: string(root["publicKey"]))!,
            devicePublicKey: Data(base64Encoded: string(device["publicKey"]))!,
            devicePairingCode: string(device["pairingCode"]),
            fingerprint: string(root["fingerprint"]),
            envelope: signed("envelope"),
            commentEnvelope: signed("commentEnvelope"),
            deviceChange: signed("deviceChange"),
            org: signed("org"),
            canonicalCases: cases,
            p256: P256Vector(
                privateKey: privateKey, publicKey: string(p256["publicKey"]), fingerprint: string(p256["fingerprint"]),
                pairingCode: string(p256["pairingCode"]), deviceId: string(p256["deviceId"]),
                commentSignature: Data(base64Encoded: string(p256["commentSignature"]))!
            )
        )
    }

    /// Whether `key` made `signature` over the text of `kind` and `canonical`.
    static func verifies(_ key: Data, signature: Data, kind: String, canonical: String) -> Bool {
        guard let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: key) else { return false }
        return publicKey.isValidSignature(signature, for: Data("agency/v1/\(kind)\n\(canonical)".utf8))
    }
}
