import AgencyProtocol
import CryptoKit
import Foundation
import LocalAuthentication
import Security

enum SecureEnclaveError: Error, CustomStringConvertible {
    case unavailable
    case accessControl(String)

    var description: String {
        switch self {
        case .unavailable: "This device has no Secure Enclave."
        case .accessControl(let reason): "The key's protection could not be set: \(reason)"
        }
    }
}

/// This device's key: a P-256 key made inside the Secure Enclave, which no process can read, and which signs
/// only after the person confirms each signature. On the iPhone that is Face ID alone (the current set of
/// faces: adding or removing one destroys the key, and the phone has to be added again). On the Mac it is
/// Touch ID or the Mac's password (`.userPresence`), because a Mac may have no Touch ID, or be used closed with
/// another keyboard; with the password as an alternative, tying the key to the current set of fingers would
/// protect nothing. The Keychain keeps an opaque handle to the key, not the key.
///
/// There is one key for each home node: a node is a separate account, and its key never signs for another.
struct SecureEnclaveSigner: DeviceSigner {
    private static let service = KeychainStore(service: "agency.device-key")

    /// What the key asks of the person for each signature.
    private static var confirmation: SecAccessControlCreateFlags {
        #if os(macOS)
        [.privateKeyUsage, .userPresence]
        #else
        [.privateKeyUsage, .biometryCurrentSet]
        #endif
    }

    private static func account(for host: String) -> String { "device-key.\(host)" }

    private let handle: Data
    let publicKey: DevicePublicKey

    /// Makes a key for the home node `host` and keeps its handle. It never replaces a handle that is kept: the Keychain refuses to add one
    /// where there is one (`KeychainStore.add`), and what to do about a key that exists and cannot be used is the person's to choose.
    static func create(host: String) throws -> SecureEnclaveSigner {
        guard SecureEnclave.isAvailable else { throw SecureEnclaveError.unavailable }
        var error: Unmanaged<CFError>?
        guard let control = SecAccessControlCreateWithFlags(
            nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, confirmation, &error
        ) else {
            throw SecureEnclaveError.accessControl(error?.takeRetainedValue().localizedDescription ?? "unknown")
        }
        let key = try SecureEnclave.P256.Signing.PrivateKey(accessControl: control)
        try service.add(key.dataRepresentation, account: account(for: host), accessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
        return try SecureEnclaveSigner(handle: key.dataRepresentation)
    }

    /// What is kept for the home node `host`: `nil` only when nothing is. A failure to read it throws.
    static func storedHandle(host: String) throws -> Data? {
        try service.read(account: account(for: host))
    }

    /// Forgets the key of the home node `host`. The next session makes a new one, and the device is added again. A Keychain that refuses
    /// throws: the key is then still there, and the caller says so instead of going on as if it were gone.
    static func deleteKey(host: String) throws {
        try service.delete(account: account(for: host))
    }

    init(handle: Data) throws {
        let key = try SecureEnclave.P256.Signing.PrivateKey(dataRepresentation: handle)
        self.handle = handle
        publicKey = try DevicePublicKey(p256X963: key.publicKey.x963Representation)
    }

    func sign(_ text: Data, reason: String) async throws -> Data {
        try await signEach([text], reason: reason)[0]
    }

    /// One `LAContext` for all of them: the person confirms once, and the key is used again with what that confirmation allowed.
    func signEach(_ texts: [Data], reason: String) async throws -> [Data] {
        let handle = handle
        // The call waits for the person to confirm, so it runs off the main actor.
        return try await Task.detached(priority: .userInitiated) {
            let context = LAContext()
            context.localizedReason = reason
            context.touchIDAuthenticationAllowableReuseDuration = 10
            let key = try SecureEnclave.P256.Signing.PrivateKey(dataRepresentation: handle, authenticationContext: context)
            return try texts.map { try key.signature(for: $0).rawRepresentation }
        }.value
    }
}
