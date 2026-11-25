import Foundation

/// The Cloudflare Access token the sign-in handed over, for the node's host.
///
/// Access sets the same token as the `CF_Authorization` cookie in a browser, and accepts it as the
/// `cf-access-token` header from other clients. While the app still shows the node in a web view,
/// it writes the cookie the way Access itself does: host-only, path `/`, `Secure`, `HttpOnly`, no
/// `SameSite`, expiring when the token does. That only works while the Access application does not
/// bind the cookie to the browser that got it, because the token comes from Safari's context.
public struct AccessToken: Codable, Equatable, Sendable {
    public static let cookieName = "CF_Authorization"
    public static let headerName = "cf-access-token"

    public let value: String
    /// The node's host, without a leading dot, which makes the cookie host-only.
    public let host: String
    public let expires: Date

    public var cookiePath: String { "/" }
    public var isCookieSecure: Bool { true }
    public var isCookieHTTPOnly: Bool { true }

    init(value: String, host: String, expires: Date) {
        self.value = value
        self.host = host
        self.expires = expires
    }
}
