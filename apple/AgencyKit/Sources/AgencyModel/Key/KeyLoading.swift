import AgencyProtocol
import Foundation

/// What reading this device's stored key handle came to. Only "there is none" lets a key be made: every other failure leaves the stored
/// handle alone, because a key that was made again under a handle that exists is a different device to the node.
public enum KeyHandleRead: Equatable, Sendable {
    case found(Data)
    /// Nothing is stored for the home node: a first launch, or after the person made the key again.
    case notFound
    /// The store did not answer, or refused: `isTransient` for a failure that another try, or the phone being unlocked, may mend.
    case failed(reason: String, isTransient: Bool)
}

/// The device's key store, as the loader needs it. The app's is the Keychain with the Secure Enclave; tests use a stand-in.
public protocol KeyVault: Sendable {
    func readHandle(host: String) -> KeyHandleRead
    /// Opens a stored handle as a signer; throws when it cannot be opened.
    func open(_ handle: Data) throws -> any DeviceSigner
    /// Makes a key and stores its handle. It never replaces a handle that is stored: it throws `KeyCreateError.alreadyStored` when there is
    /// one (stored by another run between the read and now), and the loader then reads it.
    func create(host: String) throws -> any DeviceSigner
}

/// Why this device's key is not in use, said so that nothing is lost by it: the stored handle is left as it is.
public struct KeyProblem: Equatable, Sendable {
    /// What went wrong, as the system said it; it is also written to the log.
    public let reason: String
    /// Whether trying again may mend it (the phone is locked, the store is busy), as opposed to a handle that cannot be opened.
    public let isTransient: Bool

    public init(reason: String, isTransient: Bool) {
        self.reason = reason
        self.isTransient = isTransient
    }
}

/// What loading the key came to.
public enum KeyLoad: Sendable {
    case ready(any DeviceSigner)
    /// A key is stored and cannot be used now. Nothing was replaced; the person chooses to try again or to make a new key.
    case unusable(KeyProblem)
    /// There is no key and none could be made (no Secure Enclave, or its protection could not be set): the desk can only be read.
    case cannotMake(String)
}

/// Loads this device's key for a home node: the one stored, or, only when none is stored, a new one. **A key is never replaced because
/// reading or opening the stored one failed**: such a failure is tried again a few times and then reported, and the stored handle stays.
public struct KeyLoader: Sendable {
    private let vault: any KeyVault
    private let attempts: Int
    private let pause: @Sendable () async -> Void
    private let record: @Sendable (String) -> Void

    /// - Parameters:
    ///   - attempts: how many times a failure that may pass is tried before it is reported.
    ///   - pause: waits between two tries, without holding the thread it runs on.
    ///   - record: where the reason a key was judged unusable is written.
    public init(
        vault: any KeyVault, attempts: Int = 3, pause: @escaping @Sendable () async -> Void = { try? await Task.sleep(for: .milliseconds(150)) },
        record: @escaping @Sendable (String) -> Void = { _ in }
    ) {
        self.vault = vault
        self.attempts = max(1, attempts)
        self.pause = pause
        self.record = record
    }

    public func load(host: String) async -> KeyLoad {
        await load(host: host, canMake: true)
    }

    /// `canMake` is false when a key was just found to be stored by someone else: reading it is all that is left to do.
    private func load(host: String, canMake: Bool) async -> KeyLoad {
        var last = KeyProblem(reason: "", isTransient: true)
        for attempt in 0..<attempts {
            if attempt > 0 { await pause() }
            switch vault.readHandle(host: host) {
            case .notFound:
                guard canMake else {
                    return unusable(KeyProblem(reason: "A key was stored, and then none was found.", isTransient: true))
                }
                return await make(host: host)
            case .failed(let reason, let isTransient):
                last = KeyProblem(reason: reason, isTransient: isTransient)
                guard isTransient else { return unusable(last) }
            case .found(let handle):
                do {
                    return .ready(try vault.open(handle))
                } catch {
                    // A handle that exists and does not open: tried again, since a locked phone looks the same, and never replaced.
                    last = KeyProblem(reason: "\(error)", isTransient: true)
                }
            }
        }
        return unusable(last)
    }

    private func make(host: String) async -> KeyLoad {
        do {
            return .ready(try vault.create(host: host))
        } catch KeyCreateError.alreadyStored {
            // A handle was stored meanwhile: it is the key, and it is read, not replaced and not reported as "none could be made".
            return await load(host: host, canMake: false)
        } catch {
            record("The device key could not be made: \(error)")
            return .cannotMake("\(error)")
        }
    }

    private func unusable(_ problem: KeyProblem) -> KeyLoad {
        record("The device key is stored and cannot be used: \(problem.reason)")
        return .unusable(problem)
    }
}

/// Which Keychain failures are worth another try.
public enum KeychainFailure {
    /// `errSecInteractionNotAllowed` (the phone is locked, or its protected data is not available yet), `errSecNotAvailable`,
    /// `errSecServiceNotAvailable`, `errSecIO`, `errSecAllocate` and `errSecOpWr`: they pass; any other status does not.
    public static func isTransient(status: Int32) -> Bool {
        [-25308, -25291, -67585, -36, -108, -49].contains(status)
    }
}
