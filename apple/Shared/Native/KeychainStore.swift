import Foundation
import Security

struct KeychainError: Error, CustomStringConvertible {
    let status: OSStatus

    var description: String {
        ((SecCopyErrorMessageString(status, nil) as String?) ?? "Keychain error") + " (\(status))"
    }
}

/// Generic passwords in the Keychain under one service. Nothing here syncs to iCloud or leaves this device.
struct KeychainStore: Sendable {
    let service: String

    func save(_ data: Data, account: String, accessible: CFString = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly) throws {
        let key = query(account)
        let status = SecItemUpdate(key as CFDictionary, [kSecValueData: data, kSecAttrAccessible: accessible] as CFDictionary)
        if status == errSecSuccess { return }
        guard status == errSecItemNotFound else { throw KeychainError(status: status) }
        var item = key
        item[kSecValueData] = data
        item[kSecAttrAccessible] = accessible
        let added = SecItemAdd(item as CFDictionary, nil)
        guard added == errSecSuccess else { throw KeychainError(status: added) }
    }

    /// Stores `data` only when nothing is stored under `account`: it throws (`errSecDuplicateItem`) when something is, so that a stored
    /// item is never replaced by it. `save` is for what may be replaced.
    func add(_ data: Data, account: String, accessible: CFString) throws {
        var item = query(account)
        item[kSecValueData] = data
        item[kSecAttrAccessible] = accessible
        let status = SecItemAdd(item as CFDictionary, nil)
        guard status == errSecSuccess else { throw KeychainError(status: status) }
    }

    /// What is stored under `account`: `nil` only when there is nothing (`errSecItemNotFound`). Any other failure throws, since a failure
    /// to read (a locked phone, a busy store) says nothing of whether something is there.
    func read(account: String) throws -> Data? {
        var request = query(account)
        request[kSecReturnData] = true
        request[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else { throw KeychainError(status: status == errSecSuccess ? errSecDecode : status) }
        return data
    }

    func load(account: String) -> Data? {
        var request = query(account)
        request[kSecReturnData] = true
        request[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        guard SecItemCopyMatching(request as CFDictionary, &result) == errSecSuccess else { return nil }
        return result as? Data
    }

    func delete(account: String) {
        SecItemDelete(query(account) as CFDictionary)
    }

    private func query(_ account: String) -> [CFString: Any] {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecAttrAccount: account, kSecAttrSynchronizable: false,
        ]
        #if os(macOS)
        // The Mac's default Keychain is the login keychain, which ignores `kSecAttrAccessible` and asks the person to
        // allow access; the data protection keychain behaves as the iPhone's does.
        query[kSecUseDataProtectionKeychain] = true
        #endif
        return query
    }
}
