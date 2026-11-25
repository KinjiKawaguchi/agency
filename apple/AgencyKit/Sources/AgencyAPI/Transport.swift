import Foundation

/// A request to the home node, before the client adds what every request carries.
public struct HTTPRequest: Equatable, Sendable {
    public var method: String
    /// The path with its query, such as `/api/decisions?limit=50`.
    public var path: String
    public var headers: [String: String]
    public var body: Data?

    public init(method: String, path: String, headers: [String: String] = [:], body: Data? = nil) {
        self.method = method
        self.path = path
        self.headers = headers
        self.body = body
    }
}

public struct HTTPResponse: Equatable, Sendable {
    public var status: Int
    public var body: Data

    public init(status: Int, body: Data) {
        self.status = status
        self.body = body
    }
}

/// Sends a request to the home node and answers what came back. An implementation must not follow
/// redirects: Access answers an expired session with one, and the client reads it as the end of the
/// session.
public protocol HTTPTransport: Sendable {
    func send(_ request: HTTPRequest) async throws -> HTTPResponse
}
