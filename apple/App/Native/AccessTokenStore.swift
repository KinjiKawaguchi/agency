import AgencyKit
import Foundation

/// The Access token the sign-in handed over, kept in the Keychain for the home node's host. It is what
/// the API calls send as `cf-access-token`. An expired token is never handed out.
struct AccessTokenStore: Sendable {
    private let keychain: KeychainStore
    private let host: String

    init(host: String, keychain: KeychainStore = KeychainStore(service: "agency.access-token")) {
        self.host = host
        self.keychain = keychain
    }

    func current(now: Date = Date()) -> AccessToken? {
        guard let data = keychain.load(account: host),
              let token = try? JSONDecoder().decode(AccessToken.self, from: data),
              token.host == host, token.expires > now
        else { return nil }
        return token
    }

    func save(_ token: AccessToken) {
        guard token.host == host, let data = try? JSONEncoder().encode(token) else { return }
        try? keychain.save(data, account: host)
    }

    func clear() {
        keychain.delete(account: host)
    }
}
