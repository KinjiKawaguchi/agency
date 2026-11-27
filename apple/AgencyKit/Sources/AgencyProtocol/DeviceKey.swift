import Crypto
import Foundation

public enum DeviceKeyError: Error, Equatable, Sendable {
    case malformedKey
    case malformedSignature
}

/// A device's public key as the protocol writes it: Ed25519 keys are the bare base64 of their 32
/// bytes, and P-256 keys are `p256:` and the base64 of the 65-byte uncompressed X9.63 point (the form
/// CryptoKit gives as `x963Representation`). Only the uncompressed form is accepted, so one key has
/// exactly one string, one fingerprint and one device id.
public enum DevicePublicKey: Equatable, Sendable {
    case ed25519(Data)
    case p256(Data)

    public static let p256Prefix = "p256:"

    /// Reads a key string; anything that is not exactly a key of one of the two kinds is refused.
    public init(string: String) throws {
        if string.hasPrefix(Self.p256Prefix) {
            // CryptoKit accepts only the 65-byte uncompressed point (0x04 and two 32-byte coordinates) on the curve.
            guard let bytes = Data(base64Encoded: String(string.dropFirst(Self.p256Prefix.count))),
                  (try? P256.Signing.PublicKey(x963Representation: bytes)) != nil,
                  Self.p256Prefix + bytes.base64EncodedString() == string
            else { throw DeviceKeyError.malformedKey }
            self = .p256(bytes)
        } else {
            guard let bytes = Data(base64Encoded: string), bytes.count == 32, bytes.base64EncodedString() == string else {
                throw DeviceKeyError.malformedKey
            }
            self = .ed25519(bytes)
        }
    }

    public init(p256X963 bytes: Data) throws {
        try self.init(string: Self.p256Prefix + bytes.base64EncodedString())
    }

    public var string: String {
        switch self {
        case .ed25519(let bytes): bytes.base64EncodedString()
        case .p256(let bytes): Self.p256Prefix + bytes.base64EncodedString()
        }
    }

    /// The key's own bytes (32, or the 65 of the point), which the fingerprint hashes.
    public var bytes: Data {
        switch self {
        case .ed25519(let bytes), .p256(let bytes): bytes
        }
    }

    public var fingerprint: DeviceFingerprint { DeviceFingerprint(publicKey: bytes) }

    /// Whether `signature` is this key's signature over `text`. A P-256 signature is ECDSA with SHA-256,
    /// as 64 raw bytes (`r` then `s`); an Ed25519 one is the usual 64 bytes.
    public func verifies(_ signature: Data, over text: Data) -> Bool {
        switch self {
        case .ed25519(let bytes):
            guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: bytes) else { return false }
            return key.isValidSignature(signature, for: text)
        case .p256(let bytes):
            guard let key = try? P256.Signing.PublicKey(x963Representation: bytes),
                  let ecdsa = try? P256.Signing.ECDSASignature(rawRepresentation: signature)
            else { return false }
            return key.isValidSignature(ecdsa, for: text)
        }
    }
}

/// Something that signs with this device's key. The app's implementation lives in the Secure Enclave
/// and asks for Face ID for each signature; tests use `SoftwareP256Signer`.
public protocol DeviceSigner: Sendable {
    /// This device's public key as the protocol writes it (`p256:...`).
    var publicKey: DevicePublicKey { get }

    /// Signs `text`. `reason` is what the person is shown when they are asked to confirm.
    /// - Returns: the 64 raw bytes of the signature.
    func sign(_ text: Data, reason: String) async throws -> Data

    /// Signs each of `texts` for one confirmation: a change that is sent to several orgs asks the person once, not once for each. The
    /// default signs them one by one, which is right for a signer that does not ask.
    /// - Returns: the signatures, in the order of `texts`.
    func signEach(_ texts: [Data], reason: String) async throws -> [Data]
}

extension DeviceSigner {
    public func signEach(_ texts: [Data], reason: String) async throws -> [Data] {
        var signatures: [Data] = []
        for text in texts { signatures.append(try await sign(text, reason: reason)) }
        return signatures
    }
}

/// A P-256 key held in memory, for tests and for checking the Swift signatures against the node. It
/// keeps the key's raw bytes (which are `Sendable` on every platform) and makes the key when it signs.
public struct SoftwareP256Signer: DeviceSigner {
    private let rawPrivateKey: Data
    public let publicKey: DevicePublicKey

    public init(key: P256.Signing.PrivateKey) {
        rawPrivateKey = key.rawRepresentation
        // A key CryptoKit made is always a valid point.
        publicKey = try! DevicePublicKey(p256X963: key.publicKey.x963Representation)
    }

    public init(rawPrivateKey: Data) throws {
        self.init(key: try P256.Signing.PrivateKey(rawRepresentation: rawPrivateKey))
    }

    public func sign(_ text: Data, reason: String) async throws -> Data {
        try P256.Signing.PrivateKey(rawRepresentation: rawPrivateKey).signature(for: text).rawRepresentation
    }
}
