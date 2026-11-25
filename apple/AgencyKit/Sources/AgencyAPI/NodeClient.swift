import AgencyProtocol
import Foundation

/// A decision to send: the signed envelope and the notes the decision log keeps.
public struct SignedDecision: Sendable {
    public let envelope: DecisionEnvelope
    /// The id of the signing device.
    public let signer: String
    /// The signature in standard base64.
    public let signature: String
    /// The request's title and the label of the chosen option, for people to read in the log (at most 200 characters each).
    public let title: String?
    public let choiceLabel: String?

    public init(envelope: DecisionEnvelope, signer: String, signature: String, title: String? = nil, choiceLabel: String? = nil) {
        self.envelope = envelope
        self.signer = signer
        self.signature = signature
        self.title = title
        self.choiceLabel = choiceLabel
    }

    var json: JSONValue {
        .object([
            .init("envelope", .object([
                .init("body", envelope.json), .init("signer", .string(signer)), .init("signature", .string(signature)),
            ])),
            .init("title", title.map(JSONValue.string) ?? .null),
            .init("choiceLabel", choiceLabel.map(JSONValue.string) ?? .null),
        ])
    }
}

/// The client of the home node's Client API.
///
/// Every request carries the Access token as `cf-access-token`. A write also carries the write token
/// from `GET /api/session`, fetched on first use and once more when the node says it is stale
/// (it changes when the node restarts). The client keeps the difference between the node's clock and
/// this phone's, so a signed envelope's time is the node's.
public actor NodeClient {
    public static let accessHeader = "cf-access-token"
    public static let writeHeader = "x-agency-token"
    /// What the node says when the write token is not the one it holds now. Any other 403 (the signed-in
    /// person is not the owner, say) is an answer and is not sent again.
    static let staleWriteTokenMessage = "missing or wrong write token"

    private let transport: any HTTPTransport
    private let accessToken: @Sendable () async -> String?
    private let clock: @Sendable () -> Date
    private var writeToken: String?
    private var clockOffset: TimeInterval = 0

    /// - Parameters:
    ///   - accessToken: the current Access token's value, or `nil` when the owner is not signed in or it has expired.
    ///   - clock: this phone's clock, a parameter so tests can fix it.
    public init(
        transport: any HTTPTransport,
        accessToken: @escaping @Sendable () async -> String?,
        clock: @escaping @Sendable () -> Date = Date.init
    ) {
        self.transport = transport
        self.accessToken = accessToken
        self.clock = clock
    }

    // MARK: Reads

    public func me() async throws -> Me {
        try await get("/api/me")
    }

    public func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try await get("/api/orgs/\(org)/teams/\(team)/view")
    }

    public func decisions(limit: Int = 50) async throws -> DecisionLog {
        try await get("/api/decisions?limit=\(limit)")
    }

    /// The node's time now, from the offset the last session call measured.
    public func nodeNow() -> Date {
        clock().addingTimeInterval(clockOffset)
    }

    // MARK: Writes

    /// Sends a signed decision. The envelope's time must come from `nodeNow()`.
    public func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        let body = Data(try CanonicalJSON.encode(decision.json).utf8)
        return try await write("POST", "/api/orgs/\(org)/teams/\(team)/envelopes", body: body)
    }

    /// Fetches the write token and the node's time (the clock offset is kept for `nodeNow()`).
    @discardableResult
    public func refreshSession() async throws -> Session {
        let session: Session = try await get("/api/session")
        writeToken = session.writeToken
        clockOffset = session.now.date.timeIntervalSince(clock())
        return session
    }

    // MARK: Plumbing

    private func get<Answer: Decodable>(_ path: String) async throws -> Answer {
        try decode(try await send(HTTPRequest(method: "GET", path: path)))
    }

    private func write<Answer: Decodable>(_ method: String, _ path: String, body: Data) async throws -> Answer {
        if writeToken == nil { try await refreshSession() }
        do {
            return try decode(try await send(HTTPRequest(method: method, path: path, body: body)))
        } catch NodeError.http(let status, let message) where status == 403 && message == Self.staleWriteTokenMessage {
            try await refreshSession()
            return try decode(try await send(HTTPRequest(method: method, path: path, body: body)))
        }
    }

    private func send(_ original: HTTPRequest) async throws -> HTTPResponse {
        guard let token = await accessToken() else { throw NodeError.notSignedIn }
        var request = original
        request.headers[Self.accessHeader] = token
        if request.method != "GET" {
            request.headers["content-type"] = "application/json"
            request.headers[Self.writeHeader] = writeToken
        }
        let response = try await transport.send(request)
        switch response.status {
        case 300..<400:
            throw NodeError.sessionExpired
        case 200..<300:
            return response
        default:
            throw NodeError.http(status: response.status, message: try? JSONDecoder().decode(ErrorBody.self, from: response.body).error)
        }
    }

    private func decode<Answer: Decodable>(_ response: HTTPResponse) throws -> Answer {
        do {
            return try JSONDecoder().decode(Answer.self, from: response.body)
        } catch {
            throw NodeError.unreadable("\(error)")
        }
    }
}
