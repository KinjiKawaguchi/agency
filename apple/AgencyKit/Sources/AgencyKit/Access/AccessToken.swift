import Foundation

/// The Cloudflare Access token the sign-in handed over, for the node's host. Access accepts it as the
/// `cf-access-token` header, which is how every call to the node sends it.
public struct AccessToken: Codable, Equatable, Sendable {
    public static let headerName = "cf-access-token"

    public let value: String
    /// The node's host.
    public let host: String
    public let expires: Date

    init(value: String, host: String, expires: Date) {
        self.value = value
        self.host = host
        self.expires = expires
    }
}
