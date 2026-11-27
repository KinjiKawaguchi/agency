import Foundation

/// What the Keychain's answers mean for the device key, decided apart from the Keychain so that each can be tested. The rule they keep:
/// **only "nothing is stored" lets a key be made**, and nothing that is stored is replaced.
public enum KeychainStatus {
    public static let success: Int32 = 0
    /// `errSecItemNotFound`.
    public static let itemNotFound: Int32 = -25300
    /// `errSecDuplicateItem`.
    public static let duplicateItem: Int32 = -25299
    /// `errSecDecode`: the Keychain said it succeeded and gave no data.
    public static let decode: Int32 = -26275

    public enum Read: Equatable, Sendable {
        case found
        /// Nothing is stored: the only answer that is not a failure to read.
        case notFound
        case failed(status: Int32)
    }

    /// What a read came to: `hasData` is whether the Keychain returned the data it was asked for.
    public static func read(status: Int32, hasData: Bool) -> Read {
        switch status {
        case success: hasData ? .found : .failed(status: decode)
        case itemNotFound: .notFound
        default: .failed(status: status)
        }
    }

    public enum Add: Equatable, Sendable {
        case added
        /// Something is stored already: it is left as it is.
        case duplicate
        case failed(status: Int32)
    }

    public static func add(status: Int32) -> Add {
        switch status {
        case success: .added
        case duplicateItem: .duplicate
        default: .failed(status: status)
        }
    }

    public enum Delete: Equatable, Sendable {
        case deleted
        /// Nothing was stored, so there is nothing to delete: what was asked for is true.
        case nothingToDelete
        /// The Keychain did not delete it: a key that was to be made again is still there, and the person must be told.
        case failed(status: Int32)
    }

    public static func delete(status: Int32) -> Delete {
        switch status {
        case success: .deleted
        case itemNotFound: .nothingToDelete
        default: .failed(status: status)
        }
    }
}

/// Why a key could not be made.
public enum KeyCreateError: Error, Equatable, Sendable {
    /// A handle is stored already (it was stored between the read and the add): it is read again, never replaced.
    case alreadyStored
}
