import Foundation

/// Why what came back from the sign-in session cannot be used.
public enum AccessHandoffError: Error, Equatable, Sendable {
    /// The address is not the callback of this node, or is malformed.
    case notTheCallback
    /// The `state` is not the one this sign-in started with.
    case stateMismatch
    case missingToken
    /// The token is not a JWT with an `exp` claim.
    case malformedToken
    case expired
}

/// One sign-in of the owner through the system's web authentication session, which runs in
/// Safari's context, where passkeys and password managers work.
///
/// The session starts on the node's handoff route behind Access. After Access signs the owner in,
/// the node redirects to the callback on its own host and carries the Access token and this
/// handoff's `state` in the URL fragment. The callback is a universal link, so only an app that the
/// node's association file names receives it. The callback is untrusted input: the host, the path,
/// the `state` and the token all have to check out.
public struct AccessHandoff: Equatable, Sendable {
    public static let startPath = "/app/handoff"
    public static let callbackPath = "/app/callback"

    static let stateByteCount = 32
    static let maxTokenLength = 8192

    public let host: String
    /// 43 characters of base64url: 256 bits made for this sign-in.
    public let state: String

    /// A handoff is possible only for a node on `https` port 443 whose host is the app's associated
    /// domain, because that is the only host a universal link can return to the app from.
    /// - Parameters:
    ///   - associatedHost: the host the app was built with in its associated domains.
    ///   - randomBytes: 32 bytes for the `state`; the system's generator unless a test says otherwise.
    public init?(
        homeNode: HomeNodeURL,
        associatedHost: String,
        randomBytes: () -> [UInt8] = AccessHandoff.systemRandomBytes
    ) {
        guard homeNode.scheme == "https", homeNode.port == 443,
              !associatedHost.isEmpty, associatedHost.lowercased() == homeNode.host
        else { return nil }
        let bytes = randomBytes()
        guard bytes.count == Self.stateByteCount else { return nil }
        host = homeNode.host
        state = Base64URL.encode(bytes)
    }

    public static func systemRandomBytes() -> [UInt8] {
        (0..<stateByteCount).map { _ in UInt8.random(in: .min ... .max) }
    }

    /// Where the session starts.
    public var startURL: URL {
        var components = URLComponents()
        components.scheme = "https"
        components.host = host
        components.path = Self.startPath
        components.queryItems = [URLQueryItem(name: "state", value: state)]
        return components.url!
    }

    /// The host and path the session waits for.
    public var callbackHost: String { host }
    public var callbackPath: String { Self.callbackPath }

    /// The token from the address the session returned.
    /// - Parameter now: tokens that have expired by then are refused.
    public func token(from callback: URL, now: Date) throws -> AccessToken {
        guard let components = URLComponents(url: callback, resolvingAgainstBaseURL: false),
              components.scheme?.lowercased() == "https",
              components.host?.lowercased() == host,
              (components.port ?? 443) == 443,
              components.percentEncodedPath == Self.callbackPath,
              let fragment = components.percentEncodedFragment,
              let fields = Self.fields(of: fragment)
        else { throw AccessHandoffError.notTheCallback }

        guard fields["state"] == state else { throw AccessHandoffError.stateMismatch }
        guard let token = fields["token"] else { throw AccessHandoffError.missingToken }
        let expires = try Self.expiry(ofToken: token)
        guard expires > now else { throw AccessHandoffError.expired }
        return AccessToken(value: token, host: host, expires: expires)
    }

    /// `a=1&b=2` as a dictionary, or `nil` when a name repeats or a part has no value.
    private static func fields(of fragment: String) -> [String: String]? {
        var fields: [String: String] = [:]
        for part in fragment.split(separator: "&", omittingEmptySubsequences: false) {
            let pair = part.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            guard pair.count == 2, fields[String(pair[0])] == nil else { return nil }
            fields[String(pair[0])] = String(pair[1])
        }
        return fields
    }

    /// The `exp` claim of a JWT. The signature is not checked: the node does that on every request,
    /// and the app only needs to know when to stop using the token.
    private static func expiry(ofToken token: String) throws -> Date {
        let segments = token.split(separator: ".", omittingEmptySubsequences: false)
        guard token.count <= maxTokenLength, segments.count == 3,
              segments.allSatisfy({ !$0.isEmpty && Base64URL.isAlphabet($0) }),
              let payload = Base64URL.decode(String(segments[1])),
              let claims = try? JSONDecoder().decode(Claims.self, from: payload)
        else { throw AccessHandoffError.malformedToken }
        return Date(timeIntervalSince1970: claims.exp)
    }

    private struct Claims: Decodable {
        let exp: Double
    }
}

enum Base64URL {
    private static let alphabet = Set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_".unicodeScalars)

    static func isAlphabet(_ text: Substring) -> Bool {
        text.unicodeScalars.allSatisfy(alphabet.contains)
    }

    static func encode(_ bytes: [UInt8]) -> String {
        Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    static func decode(_ text: String) -> Data? {
        var base64 = text.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        base64 += String(repeating: "=", count: (4 - base64.count % 4) % 4)
        return Data(base64Encoded: base64)
    }
}
