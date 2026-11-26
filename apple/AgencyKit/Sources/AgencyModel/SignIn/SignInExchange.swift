import AgencyAPI
import AgencyKit
import AgencyProtocol
import Foundation

/// Why what came back from the sign-in session cannot be used.
public enum SignInExchangeError: Error, Equatable, Sendable {
    /// The address is not this sign-in's callback, or is malformed.
    case notTheCallback
    /// The `state` is not the one this sign-in started with.
    case stateMismatch
    /// The callback has no code, or one that is not 43 base64url characters.
    case missingCode
    /// This sign-in already gave its code: a callback delivered twice redeems once.
    case alreadyUsed
    /// The sign-in was started too long ago for the owner still to be at it.
    case expired
}

/// What redeems a code: the node's client. A stand-in in tests.
public protocol SignInRedeeming: Sendable {
    func redeemSignIn(code: String, verifier: String) async throws -> String
}

extension NodeClient: SignInRedeeming {}

/// One sign-in of the owner to a home node through the system's web authentication session, which runs in Safari's
/// context, where passkeys and password managers work, and for any host.
///
/// The app makes a random `verifier` and starts the session on the node's handoff route with its SHA-256 as the
/// `challenge`. After Access signs the owner in, the node redirects to the callback with a single-use `code` and this
/// sign-in's `state` in the fragment: **no token**. The app then redeems the code with the verifier
/// (`NodeClient.redeemSignIn`), and only then has the token. A callback that someone else read is useless: the code
/// is worth nothing without the verifier, which never left the app.
///
/// The callback comes back one of two ways (`Transport`): a universal link on the node's own host, which only the app
/// the node's association file names can receive, for a host the build has an associated domain for; or the custom
/// scheme `agency-signin`, which needs no domain, for any other. The callback is untrusted input.
public final class SignInExchange: @unchecked Sendable, CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    public enum Transport: Equatable, Sendable {
        case universalLink
        case customScheme
    }

    public static let startPath = "/app/handoff"
    public static let callbackPath = "/app/callback"
    public static let callbackScheme = "agency-signin"
    /// How long after it started a sign-in can still give its code.
    public static let lifetime: TimeInterval = 10 * 60

    static let byteCount = 32
    static let secretLength = 43

    public let host: String
    public let transport: Transport
    /// 43 characters of base64url: 256 bits made for this sign-in; it ties the callback to it.
    let state: String
    /// 43 characters of base64url: 256 bits that never leave the app until the code is redeemed (`redeem`).
    let verifier: String
    /// base64url of the SHA-256 of `verifier`, as the node expects it.
    let challenge: String
    private let port: Int
    private let startedAt: Date
    private let now: () -> Date
    private let lock = NSLock()
    private var isUsed = false

    /// A sign-in is possible for a node on `https` (Access is only on https). The callback is a universal link when
    /// the host is one of the build's associated domains, and the custom scheme otherwise.
    /// - Parameters:
    ///   - associatedHosts: what the build has in its associated domains: a host (`node.example.com`) or a
    ///     wildcard (`*.example.com`, which matches one label below `example.com` and not `example.com` itself).
    ///   - randomBytes: 32 bytes each for the `state` and the `verifier`; the system's generator unless a test says.
    ///   - now: the clock, for the lifetime; a test hands in its own.
    public init?(
        homeNode: HomeNodeURL,
        associatedHosts: [String],
        randomBytes: () -> [UInt8] = SignInExchange.systemRandomBytes,
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        guard homeNode.scheme == "https" else { return nil }
        let stateBytes = randomBytes()
        let verifierBytes = randomBytes()
        guard stateBytes.count == Self.byteCount, verifierBytes.count == Self.byteCount else { return nil }
        host = homeNode.host
        port = homeNode.port
        state = Base64URL.encode(stateBytes)
        verifier = Base64URL.encode(verifierBytes)
        challenge = Base64URL.encode(Array(Sha256.digest(Data(verifier.utf8))))
        let isAssociated = homeNode.port == 443 && Self.isAssociated(host: homeNode.host, with: associatedHosts)
        transport = isAssociated ? .universalLink : .customScheme
        self.now = now
        startedAt = now()
    }

    /// What a print, a log line or a debugger says of it: where it signs in, and nothing that is secret.
    public var description: String { "SignInExchange(host: \(host), transport: \(transport))" }
    public var debugDescription: String { description }
    public var customMirror: Mirror { Mirror(self, children: ["host": host, "transport": "\(transport)"], displayStyle: .class) }

    public static func systemRandomBytes() -> [UInt8] {
        (0..<byteCount).map { _ in UInt8.random(in: .min ... .max) }
    }

    /// Whether a universal link can come back from `host`: it is one of the associated domains, or one label below a
    /// wildcard one.
    static func isAssociated(host: String, with associatedHosts: [String]) -> Bool {
        associatedHosts.contains { entry in
            let entry = entry.lowercased()
            guard !entry.isEmpty else { return false }
            guard entry.hasPrefix("*.") else { return entry == host }
            let suffix = String(entry.dropFirst(1))
            guard host.hasSuffix(suffix) else { return false }
            // What is left of the host is not empty (a host never starts with a dot) and is a single label.
            return !host.dropLast(suffix.count).contains(".")
        }
    }

    /// Where the session starts.
    public var startURL: URL {
        var components = URLComponents()
        components.scheme = "https"
        components.host = host
        if port != 443 { components.port = port }
        components.path = Self.startPath
        components.queryItems = [
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "challenge", value: challenge),
            URLQueryItem(name: "via", value: transport == .universalLink ? "link" : "scheme"),
        ]
        return components.url!
    }

    /// What the session waits for, for `ASWebAuthenticationSession.Callback`: a custom scheme, or an https host and path.
    public var callbackScheme: String? { transport == .customScheme ? Self.callbackScheme : nil }
    public var callbackHost: String? { transport == .universalLink ? host : nil }
    public var callbackPath: String { Self.callbackPath }

    /// The code from the address the session returned, once. Only this sign-in's own callback gives it: another address
    /// (a page that opens `agency-signin://callback#...`, a callback of another sign-in) is refused and does **not**
    /// use the sign-in up, or anyone could cancel it. Once the code has been given, a second delivery of the same
    /// callback (the session's completion and the app's URL handler both see it) is refused too.
    public func code(from callback: URL) throws -> String {
        guard let components = URLComponents(url: callback, resolvingAgainstBaseURL: false), isOurCallback(components),
              let fragment = components.percentEncodedFragment,
              let fields = Self.fields(of: fragment), fields.count == 2
        else { throw SignInExchangeError.notTheCallback }
        guard fields["state"] == state else { throw SignInExchangeError.stateMismatch }
        guard let code = fields["code"], code.count == Self.secretLength, Base64URL.isAlphabet(Substring(code)) else {
            throw SignInExchangeError.missingCode
        }
        return try lock.withLock {
            guard !isUsed else { throw SignInExchangeError.alreadyUsed }
            guard now().timeIntervalSince(startedAt) <= Self.lifetime else { throw SignInExchangeError.expired }
            isUsed = true
            return code
        }
    }

    /// Redeems the code at the node with the verifier, which goes nowhere else.
    public func redeem(code: String, using redeemer: any SignInRedeeming) async throws -> String {
        try await redeemer.redeemSignIn(code: code, verifier: verifier)
    }

    private func isOurCallback(_ components: URLComponents) -> Bool {
        switch transport {
        case .universalLink:
            components.scheme?.lowercased() == "https" && components.host?.lowercased() == host
                && (components.port ?? 443) == 443 && components.percentEncodedPath == Self.callbackPath
                && components.percentEncodedQuery == nil
        case .customScheme:
            components.scheme?.lowercased() == Self.callbackScheme && components.host?.lowercased() == "callback"
                && components.port == nil && components.percentEncodedPath.isEmpty && components.user == nil
                && components.percentEncodedQuery == nil
        }
    }

    /// `a=1&b=2` as a dictionary, or `nil` when a name repeats or a part has no value; the callback has exactly two.
    private static func fields(of fragment: String) -> [String: String]? {
        var fields: [String: String] = [:]
        for part in fragment.split(separator: "&", omittingEmptySubsequences: false) {
            let pair = part.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            guard pair.count == 2, fields[String(pair[0])] == nil else { return nil }
            fields[String(pair[0])] = String(pair[1])
        }
        return fields
    }
}
