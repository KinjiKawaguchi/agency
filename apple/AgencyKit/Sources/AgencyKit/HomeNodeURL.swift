import Foundation

/// Why a string the owner typed cannot be used as the home node address.
public enum HomeNodeURLError: Error, Equatable, Sendable {
    case empty
    case invalid
    case unsupportedScheme
    case hasCredentials
    /// Plain `http` is only accepted for the machine itself and `.local` names.
    case insecureRemoteHost
}

/// The address of the owner's home node: scheme, host and port, nothing else.
///
/// Whatever the owner pastes (a bare host name, a deep link into the web client) is reduced to the
/// origin, because the app always opens the node's front page.
public struct HomeNodeURL: Equatable, Sendable {
    public let scheme: String
    public let host: String
    /// The port in use, with the scheme's default filled in.
    public let port: Int

    /// The address of the node.
    public let url: URL

    public init(validating input: String) throws {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { throw HomeNodeURLError.empty }

        let withScheme = trimmed.contains("://") ? trimmed : "https://" + trimmed
        guard let components = URLComponents(string: withScheme), let rawHost = components.host else {
            throw HomeNodeURLError.invalid
        }
        let scheme = (components.scheme ?? "").lowercased()
        guard scheme == "https" || scheme == "http" else { throw HomeNodeURLError.unsupportedScheme }
        if components.user != nil || components.password != nil { throw HomeNodeURLError.hasCredentials }

        let host = rawHost.lowercased()
        guard Self.isPlainHostName(host) else { throw HomeNodeURLError.invalid }
        if scheme == "http" && !Self.isLocalHost(host) { throw HomeNodeURLError.insecureRemoteHost }

        let port = components.port ?? Self.defaultPort(forScheme: scheme)
        guard (1...65535).contains(port) else { throw HomeNodeURLError.invalid }

        var origin = URLComponents()
        origin.scheme = scheme
        origin.host = host
        if components.port != nil { origin.port = port }
        origin.path = "/"
        guard let url = origin.url else { throw HomeNodeURLError.invalid }

        self.scheme = scheme
        self.host = host
        self.port = port
        self.url = url
    }

    /// `scheme://host[:port]` as a browser reports it in `location.origin`: the port is left out when
    /// it is the scheme's default.
    public var origin: String {
        let isDefaultPort = port == Self.defaultPort(forScheme: scheme)
        return isDefaultPort ? "\(scheme)://\(host)" : "\(scheme)://\(host):\(port)"
    }

    /// The port a URL with this scheme reaches when it names none.
    public static func defaultPort(forScheme scheme: String) -> Int {
        scheme.lowercased() == "http" ? 80 : 443
    }

    /// Letters, digits and hyphens in dot-separated labels. Internationalized names are accepted in
    /// their punycode form only.
    private static func isPlainHostName(_ host: String) -> Bool {
        let hostNameScalars = Set("abcdefghijklmnopqrstuvwxyz0123456789-".unicodeScalars)
        let labels = host.split(separator: ".", omittingEmptySubsequences: false)
        return !labels.isEmpty && labels.allSatisfy { label in
            !label.isEmpty && !label.hasPrefix("-") && !label.hasSuffix("-")
                && label.unicodeScalars.allSatisfy(hostNameScalars.contains)
        }
    }

    private static func isLocalHost(_ host: String) -> Bool {
        host == "localhost" || host == "127.0.0.1" || host.hasSuffix(".local")
    }
}
