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
    public static let clientHeader = "x-agency-client"
    /// What the node says when the write token is not the one it holds now. Any other 403 (the signed-in
    /// person is not the owner, say) is an answer and is not sent again.
    static let staleWriteTokenMessage = "missing or wrong write token"

    private let transport: any HTTPTransport
    private let accessToken: @Sendable () async -> String?
    private let clock: @Sendable () -> Date
    /// The `x-agency-client` header's value, sent on every call; `nil` sends none.
    private let clientIdentity: String?
    private var writeToken: String?
    private var clockOffset: TimeInterval = 0

    /// - Parameters:
    ///   - accessToken: the current Access token's value, or `nil` when the owner is not signed in or it has expired.
    ///   - clock: this phone's clock, a parameter so tests can fix it.
    ///   - clientIdentity: what the app says of itself on every call (`ClientIdentity.current()`): its platform, version and build.
    public init(
        transport: any HTTPTransport,
        accessToken: @escaping @Sendable () async -> String?,
        clock: @escaping @Sendable () -> Date = Date.init,
        clientIdentity: String? = nil
    ) {
        self.transport = transport
        self.accessToken = accessToken
        self.clock = clock
        self.clientIdentity = clientIdentity
    }

    /// The header every call to the node carries besides its own, when the app has an identity to tell.
    private func identified(_ request: HTTPRequest) -> HTTPRequest {
        var request = request
        if let clientIdentity { request.headers[Self.clientHeader] = clientIdentity }
        return request
    }

    // MARK: Reads

    public func me() async throws -> Me {
        try await get("/api/me")
    }

    public func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot {
        try await get("/api/orgs/\(Self.segment(org))/teams/\(Self.segment(team))/view")
    }

    /// The team's token use over the last `days` days. A team whose runtime does not report usage answers 404.
    public func usage(org: String, team: String, days: Int) async throws -> TokenUsage {
        try await get("/api/orgs/\(Self.segment(org))/teams/\(Self.segment(team))/usage?days=\(days)")
    }

    /// How fast each team of an org ships over the last `days` days (1 to 90). A node that does not serve it answers 404.
    public func evaluation(org: String, days: Int) async throws -> Evaluation {
        try await get("/api/orgs/\(Self.segment(org))/evaluation?days=\(days)")
    }

    /// The node's own response times over the last `days` days (1 to 30). Owner only: anyone else is told 403, and a node that does
    /// not measure 404. `hourBuckets` asks for the buckets of each hour too (`hourBuckets=1`), so that the hours of several series can be
    /// added and the percentile of their whole read: only for a range with hours (up to 7 days), which is what the trend draws from.
    public func metrics(days: Int, hourBuckets: Bool = false) async throws -> Metrics {
        try await get("/api/metrics?days=\(days)" + (hourBuckets ? "&hourBuckets=1" : ""))
    }

    /// Where each team of an org spent its tokens over the last `days` days (1 to 90), by work type.
    public func effort(org: String, days: Int) async throws -> Effort {
        try await get("/api/orgs/\(Self.segment(org))/effort?days=\(days)")
    }

    /// The numbers the owner keeps for what the org measures over the last `days` days (1 to 90). Owner only: anyone else
    /// gets 403, and a client then shows nothing of it.
    public func outcomes(org: String, days: Int) async throws -> Outcomes {
        try await get("/api/orgs/\(Self.segment(org))/outcomes?days=\(days)")
    }

    /// How each KPI an aim of the org names stood around the day its epic shipped. Owner only, like `outcomes`.
    public func readings(org: String) async throws -> Readings {
        try await get("/api/orgs/\(Self.segment(org))/outcomes/readings")
    }

    /// What the org intends for each of its teams, from its definition. A team without a valid plan has none here.
    public func orgPlans(org: String) async throws -> OrgPlans {
        try await get("/api/orgs/\(Self.segment(org))")
    }

    /// The org's definition: its departments, members and the teams placed in them.
    public func orgDefinition(org: String) async throws -> OrgDefinition {
        try await get("/api/orgs/\(Self.segment(org))")
    }

    public func decisions(limit: Int = 50) async throws -> DecisionLog {
        try await get("/api/decisions?limit=\(limit)")
    }

    public func devices() async throws -> DeviceList {
        try await get("/api/devices")
    }

    /// The node's time now, from the offset the last session call measured.
    public func nodeNow() -> Date {
        clock().addingTimeInterval(clockOffset)
    }

    // MARK: Writes

    /// Sends a signed decision. The envelope's time must come from `nodeNow()`.
    public func submit(_ decision: SignedDecision, org: String, team: String) async throws -> DecisionResult {
        let body = Data(try CanonicalJSON.encode(decision.json).utf8)
        return try await write("POST", "/api/orgs/\(Self.segment(org))/teams/\(Self.segment(team))/envelopes", body: body)
    }

    /// Asks the node to add this device. `label` is 1 to 80 characters. The answer carries the pairing
    /// code to show; an existing device (or the node's terminal) adds it after the person compares codes.
    public func requestDevice(label: String, publicKey: String) async throws -> DeviceRequest {
        let body = Data(try CanonicalJSON.encode(.object([.init("label", .string(label)), .init("publicKey", .string(publicKey))])).utf8)
        return try await write("POST", "/api/devices/requests", body: body)
    }

    /// Sends a device change signed by one of this person's devices. The change's time must come from `nodeNow()`.
    public func submitDeviceChange(_ change: SignedDeviceChange) async throws -> DeviceChangeResult {
        let body = Data(try CanonicalJSON.encode(change.json).utf8)
        return try await write("POST", "/api/devices/changes", body: body)
    }

    /// Fetches the write token and the node's time (the clock offset is kept for `nodeNow()`).
    @discardableResult
    public func refreshSession() async throws -> Session {
        let session: Session = try await get("/api/session")
        writeToken = session.writeToken
        clockOffset = session.now.date.timeIntervalSince(clock())
        return session
    }

    /// Tells the node this phone's push token (`PushTokenInput`). Registering a known token updates it.
    /// `environment` is `production` or `sandbox`, as the app was signed; `label` is 1 to 80 characters.
    /// `events` names the kinds the token wants (`request`, `device`, `decision`); without it the node sends
    /// requests only, and an empty list means none.
    @discardableResult
    public func registerPushToken(
        token: String, environment: String, label: String, events: [String]? = nil
    ) async throws -> RegisteredPushToken {
        var members: [JSONValue.Member] = [
            .init("token", .string(token)), .init("environment", .string(environment)), .init("label", .string(label)),
        ]
        if let events { members.append(.init("events", .array(events.map { .string($0) }))) }
        let body = Data(try CanonicalJSON.encode(.object(members)).utf8)
        return try await write("POST", "/api/push/tokens", body: body)
    }

    /// Enters one number of a KPI for a day (`POST /api/orgs/<org>/outcomes/observations`): owner only, with the write token. A second
    /// number for the same KPI and day replaces the first in every reading, and the first stays in the history. The node sets who
    /// entered it and when. The body is plain JSON, not canonical JSON: that has integers only, and a number here is a decimal.
    @discardableResult
    public func enterNumber(org: String, input: ObservationInput) async throws -> KpiObservation {
        let body = try JSONEncoder().encode(input)
        return try await write("POST", "/api/orgs/\(Self.segment(org))/outcomes/observations", body: body)
    }

    /// Every number entered for a KPI, newest first, the ones a later entry replaced included (`GET .../outcomes/observations`; owner
    /// only). `limit` is 1 to 500.
    public func numberHistory(org: String, kpi: String, limit: Int = 100) async throws -> [KpiObservation] {
        try await get("/api/orgs/\(Self.segment(org))/outcomes/observations?kpi=\(Self.segment(kpi))&limit=\(limit)")
    }

    /// The owner's registered push tokens, each shortened to its last 8 characters, with when APNs last took a notice for it and when it
    /// last did not (`GET /api/push/tokens`).
    public func pushTokens() async throws -> [RegisteredPushToken] {
        let list: PushTokenList = try await get("/api/push/tokens")
        return list.tokens
    }

    /// Tells the node to stop sending to this push token (`DELETE /api/push/tokens/<token>`; the node answers 204, also for a
    /// token it does not know).
    public func unregisterPushToken(_ token: String) async throws {
        _ = try await writeResponse("DELETE", "/api/push/tokens/\(Self.segment(token))", body: nil)
    }

    /// What the node says in a refusal, shown with its hidden characters as `?` (it can name a label or a title): this is what an alert reads.
    private static func refusalText(_ response: HTTPResponse) -> String? {
        (try? JSONDecoder().decode(ErrorBody.self, from: response.body).error).flatMap { $0 }.map(NodeLabel.shownText)
    }

    /// The Access token a sign-in made for `code`, given the verifier whose SHA-256 the sign-in was started with
    /// (`POST /app/redeem`). It needs no Access token and no write token: the app has none yet, which is what it asks
    /// for, and the node admits the code and the verifier. A refusal is the same for every cause (`NodeError.http`
    /// with status 400), a limit is 429, and a redirect means the node's Access policy does not let this route through.
    public func redeemSignIn(code: String, verifier: String) async throws -> String {
        let body = Data(try CanonicalJSON.encode(.object([.init("code", .string(code)), .init("verifier", .string(verifier))])).utf8)
        var request = HTTPRequest(method: "POST", path: "/app/redeem", body: body)
        request.headers["content-type"] = "application/json"
        let response = try await transport.send(identified(request))
        switch response.status {
        case 200..<300:
            return (try decode(response) as SignInToken).token
        case 300..<400:
            throw NodeError.http(status: response.status, message: "the node's Access policy does not let the sign-in exchange through")
        default:
            throw NodeError.http(status: response.status, message: Self.refusalText(response))
        }
    }

    // MARK: Plumbing

    /// One path segment, escaped so that an id from the node can never add a segment or a query.
    static func segment(_ id: String) -> String {
        id.addingPercentEncoding(withAllowedCharacters: CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")) ?? ""
    }

    private func get<Answer: Decodable>(_ path: String) async throws -> Answer {
        try decode(try await send(HTTPRequest(method: "GET", path: path)))
    }

    private func write<Answer: Decodable>(_ method: String, _ path: String, body: Data) async throws -> Answer {
        try decode(try await writeResponse(method, path, body: body))
    }

    /// A write with the write token, made again once with a fresh one when the node says the token is stale.
    private func writeResponse(_ method: String, _ path: String, body: Data?) async throws -> HTTPResponse {
        if writeToken == nil { try await refreshSession() }
        do {
            return try await send(HTTPRequest(method: method, path: path, body: body))
        } catch NodeError.http(let status, let message) where status == 403 && message == Self.staleWriteTokenMessage {
            try await refreshSession()
            return try await send(HTTPRequest(method: method, path: path, body: body))
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
        let response = try await transport.send(identified(request))
        switch response.status {
        case 300..<400:
            throw NodeError.sessionExpired
        case 200..<300:
            return response
        default:
            throw NodeError.http(status: response.status, message: Self.refusalText(response))
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

private struct SignInToken: Decodable {
    let token: String
}
