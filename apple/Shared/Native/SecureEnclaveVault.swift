import AgencyModel
import AgencyProtocol
import Foundation
import os

/// This device's key store: the Keychain's handle, and the Secure Enclave key it opens. The loader (`KeyLoader`) decides what a failure
/// means; this only reports what happened and never replaces a handle.
struct SecureEnclaveVault: KeyVault {
    static let logger = Logger(subsystem: "agency.app", category: "key")

    /// The loader the app uses: a key that cannot be used is written to the log with the reason (a status and its message, never a key).
    static var loader: KeyLoader {
        KeyLoader(vault: SecureEnclaveVault(), record: { logger.error("\($0, privacy: .public)") })
    }

    func readHandle(host: String) -> KeyHandleRead {
        do {
            return try SecureEnclaveSigner.storedHandle(host: host).map(KeyHandleRead.found) ?? .notFound
        } catch let failure as KeychainError {
            return .failed(reason: failure.description, isTransient: KeychainFailure.isTransient(status: failure.status))
        } catch {
            return .failed(reason: "\(error)", isTransient: false)
        }
    }

    func open(_ handle: Data) throws -> any DeviceSigner {
        try SecureEnclaveSigner(handle: handle)
    }

    func create(host: String) throws -> any DeviceSigner {
        try SecureEnclaveSigner.create(host: host)
    }
}
