import AgencyProtocol
import Foundation
import Testing
@testable import AgencyAPI

/// A transport that answers from a script and remembers what it was asked.
final class ScriptedTransport: HTTPTransport, @unchecked Sendable {
    private let lock = NSLock()
    private var answers: [(String, HTTPResponse)]
    private var sent: [HTTPRequest] = []

    init(_ answers: [(String, HTTPResponse)]) { self.answers = answers }

    var requests: [HTTPRequest] { lock.withLock { sent } }

    func send(_ request: HTTPRequest) async throws -> HTTPResponse {
        lock.withLock {
            sent.append(request)
            let key = "\(request.method) \(request.path)"
            guard let index = answers.firstIndex(where: { $0.0 == key }) else {
                return HTTPResponse(status: 599, body: Data("no answer scripted for \(key)".utf8))
            }
            return answers.remove(at: index).1
        }
    }
}

func json(_ text: String, status: Int = 200) -> HTTPResponse { HTTPResponse(status: status, body: Data(text.utf8)) }
func fixture(_ name: String, status: Int = 200) -> HTTPResponse { HTTPResponse(status: status, body: try! Fixtures.data(name)) }

@Suite("NodeClient")
struct NodeClientTests {
    private let phone = Date(timeIntervalSince1970: 1_795_521_000)
    private let session = #"{"writeToken":"w-1","now":"2026-11-24T12:00:00Z"}"#
    private let envelope = DecisionEnvelope(
        id: "00112233445566778899aabbccddeeff", kind: .answer, org: "acme", team: "app", target: "r-1",
        text: "後で", choice: nil, account: "alice", at: "2026-11-24T12:00:00Z"
    )

    private func client(_ transport: ScriptedTransport, token: String? = "access-jwt") -> NodeClient {
        let phone = phone
        return NodeClient(transport: transport, accessToken: { token }, clock: { phone })
    }

    private var decision: SignedDecision {
        SignedDecision(envelope: envelope, signer: "d-1", signature: "c2ln", title: "Make a key?", choiceLabel: nil)
    }

    @Test("a read sends the Access token and no write token")
    func readHeaders() async throws {
        let transport = ScriptedTransport([("GET /api/me", fixture("me"))])
        let me = try await client(transport).me()
        #expect(me.account.id == "alice")
        let request = try #require(transport.requests.first)
        #expect(request.headers["cf-access-token"] == "access-jwt")
        #expect(request.headers["x-agency-token"] == nil)
        #expect(request.body == nil)
    }

    private func identified(_ transport: ScriptedTransport, identity: String?) -> NodeClient {
        let phone = phone
        return NodeClient(transport: transport, accessToken: { "access-jwt" }, clock: { phone }, clientIdentity: identity)
    }

    @Test("every call carries the app's identity: a read, the session, a write, and the write again after a stale token")
    func clientHeaderOnEveryCall() async throws {
        let transport = ScriptedTransport([
            ("GET /api/me", fixture("me")),
            ("GET /api/session", json(session)),
            ("POST /api/push/tokens", json(#"{"error":"missing or wrong write token"}"#, status: 403)),
            ("GET /api/session", json(#"{"writeToken":"w-2","now":"2026-11-24T12:00:00Z"}"#)),
            ("POST /api/push/tokens", fixture("push-token")),
        ])
        let client = identified(transport, identity: "ios/0.3.1 (202611261830)")
        _ = try await client.me()
        _ = try? await client.registerPushToken(token: "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12", environment: "production", label: "iPhone", events: ["request"])
        #expect(transport.requests.count >= 4)
        for request in transport.requests {
            #expect(request.headers["x-agency-client"] == "ios/0.3.1 (202611261830)", "\(request.method) \(request.path)")
        }
    }

    @Test("stopping a push token carries the identity on the session read and on the delete")
    func clientHeaderOnUnregister() async throws {
        let transport = ScriptedTransport([("GET /api/session", json(session)), ("DELETE /api/push/tokens/ab12", json("", status: 204))])
        try await identified(transport, identity: "ios/0.3.1 (1)").unregisterPushToken("ab12")
        #expect(transport.requests.count == 2)
        for request in transport.requests { #expect(request.headers["x-agency-client"] == "ios/0.3.1 (1)", "\(request.method) \(request.path)") }
    }

    @Test("with no identity no header is sent")
    func noClientHeader() async throws {
        let transport = ScriptedTransport([("GET /api/me", fixture("me"))])
        _ = try await identified(transport, identity: nil).me()
        #expect(try #require(transport.requests.first).headers["x-agency-client"] == nil)
    }

    @Test("every read goes to the path the node serves")
    func paths() async throws {
        let transport = ScriptedTransport([
            ("GET /api/orgs/acme/teams/app/view", fixture("team-snapshot-local")),
            ("GET /api/decisions?limit=20", fixture("decision-log")),
        ])
        let node = client(transport)
        _ = try await node.teamSnapshot(org: "acme", team: "app")
        _ = try await node.decisions(limit: 20)
        #expect(transport.requests.map(\.path) == ["/api/orgs/acme/teams/app/view", "/api/decisions?limit=20"])
    }

    @Test("without an Access token nothing is sent")
    func notSignedIn() async {
        let transport = ScriptedTransport([])
        await #expect(throws: NodeError.notSignedIn) { try await client(transport, token: nil).me() }
        #expect(transport.requests.isEmpty)
    }

    @Test("a redirect, which is Access ending the session, is not followed")
    func redirect() async {
        let transport = ScriptedTransport([("GET /api/me", json("", status: 302))])
        await #expect(throws: NodeError.sessionExpired) { try await client(transport).me() }
        #expect(transport.requests.count == 1)
    }

    @Test("the edges of the redirect and success statuses", arguments: [(299, true), (300, false), (399, false), (199, false)])
    func statusEdges(status: Int, isSuccess: Bool) async {
        let transport = ScriptedTransport([("GET /api/me", HTTPResponse(status: status, body: (try? Fixtures.data("me")) ?? Data()))])
        if isSuccess {
            #expect((try? await client(transport).me()) != nil)
        } else if (300..<400).contains(status) {
            await #expect(throws: NodeError.sessionExpired) { try await client(transport).me() }
        } else {
            await #expect(throws: NodeError.http(status: status, message: nil)) { try await client(transport).me() }
        }
    }

    @Test("an error status carries the node's text")
    func errorBody() async {
        let transport = ScriptedTransport([("GET /api/me", fixture("error", status: 404))])
        await #expect(throws: NodeError.http(status: 404, message: "not found")) { try await client(transport).me() }
        let other = ScriptedTransport([("GET /api/me", json("<html>", status: 502))])
        await #expect(throws: NodeError.http(status: 502, message: nil)) { try await client(other).me() }
    }

    @Test("an answer that is not the documented shape is unreadable, not a crash")
    func unreadable() async {
        let transport = ScriptedTransport([("GET /api/me", json("{\"account\":1}"))])
        await #expect(throws: NodeError.self) { try await client(transport).me() }
    }

    @Test("a write fetches the write token first and sends both tokens and a JSON body")
    func writeHeaders() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
        ])
        let result = try await client(transport).submit(decision, org: "acme", team: "app")
        #expect(result.outcome == .applied)
        #expect(transport.requests.map(\.path) == ["/api/session", "/api/orgs/acme/teams/app/envelopes"])
        let post = transport.requests[1]
        #expect(post.headers["x-agency-token"] == "w-1")
        #expect(post.headers["cf-access-token"] == "access-jwt")
        #expect(post.headers["content-type"] == "application/json")
        #expect(transport.requests[0].headers["x-agency-token"] == nil)
    }

    @Test("the body is the signed envelope with the log notes, and no null for a missing note")
    func writeBody() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
        ])
        _ = try await client(transport).submit(decision, org: "acme", team: "app")
        let body = String(decoding: try #require(transport.requests[1].body), as: UTF8.self)
        let expected = #"{"envelope":{"body":"# + (try CanonicalJSON.encode(envelope.json)) + #","signature":"c2ln","signer":"d-1"},"title":"Make a key?"}"#
        #expect(body == expected)
    }

    @Test("a decision without notes sends no title and no choice label; with both it sends both")
    func notes() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
        ])
        let node = client(transport)
        _ = try await node.submit(SignedDecision(envelope: envelope, signer: "d-1", signature: "c2ln"), org: "acme", team: "app")
        _ = try await node.submit(
            SignedDecision(envelope: envelope, signer: "d-1", signature: "c2ln", title: "T", choiceLabel: "ア"), org: "acme", team: "app"
        )
        let bodies = transport.requests.compactMap(\.body).map { String(decoding: $0, as: UTF8.self) }
        #expect(!bodies[0].contains("title") && !bodies[0].contains("choiceLabel"))
        #expect(bodies[1].contains(#""choiceLabel":"ア""#) && bodies[1].contains(#""title":"T""#))
    }

    @Test("the write token is fetched once and kept")
    func tokenKept() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
        ])
        let node = client(transport)
        _ = try await node.submit(decision, org: "acme", team: "app")
        _ = try await node.submit(decision, org: "acme", team: "app")
        #expect(transport.requests.filter { $0.path == "/api/session" }.count == 1)
    }

    @Test("a 403 on a write fetches the token again once and sends the write again with the new one")
    func refused() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", json(#"{"error":"missing or wrong write token"}"#, status: 403)),
            ("GET /api/session", json(#"{"writeToken":"w-2","now":"2026-11-24T12:00:00Z"}"#)),
            ("POST /api/orgs/acme/teams/app/envelopes", fixture("decision-result")),
        ])
        let result = try await client(transport).submit(decision, org: "acme", team: "app")
        #expect(result.outcome == .applied)
        #expect(transport.requests.map(\.method) == ["GET", "POST", "GET", "POST"])
        #expect(transport.requests[1].headers["x-agency-token"] == "w-1")
        #expect(transport.requests[3].headers["x-agency-token"] == "w-2")
    }

    @Test("the device list is a read; a device request is a write with the label and the key")
    func devices() async throws {
        let transport = ScriptedTransport([
            ("GET /api/devices", fixture("device-list")),
            ("GET /api/session", json(session)),
            ("POST /api/devices/requests", fixture("device-request")),
        ])
        let node = client(transport)
        #expect(try await node.devices().requests.count == 1)
        let request = try await node.requestDevice(label: "iPhone", publicKey: "p256:AAAA")
        #expect(request.pairingCode == "506e-f187-9d74")
        let post = try #require(transport.requests.last)
        #expect(post.headers["x-agency-token"] == "w-1")
        #expect(String(decoding: try #require(post.body), as: UTF8.self) == #"{"label":"iPhone","publicKey":"p256:AAAA"}"#)
        #expect(transport.requests.first?.headers["x-agency-token"] == nil)
    }

    @Test("the node's measurements are asked for with the buckets of each hour only when the reader wants them")
    func metricsQuery() async throws {
        let answer = #"{"from":"2026-11-24T10:00:00Z","until":"2026-11-24T12:00:00Z","generatedAt":"2026-11-24T11:30:00Z","series":[]}"#
        let transport = ScriptedTransport([("GET /api/metrics?days=7&hourBuckets=1", json(answer)), ("GET /api/metrics?days=30", json(answer))])
        let node = client(transport)
        _ = try await node.metrics(days: 7, hourBuckets: true)
        _ = try await node.metrics(days: 30)
        #expect(transport.requests.map(\.path) == ["/api/metrics?days=7&hourBuckets=1", "/api/metrics?days=30"])
    }

    @Test("registering a push token is a write with the token, environment and label")
    func pushToken() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/push/tokens", fixture("push-token")),
        ])
        let registered = try await client(transport).registerPushToken(token: "ab12", environment: "sandbox", label: "iPhone")
        #expect(registered.environment == .production)
        #expect(registered.events == ["request"])
        let post = try #require(transport.requests.last)
        #expect(String(decoding: try #require(post.body), as: UTF8.self) == #"{"environment":"sandbox","label":"iPhone","token":"ab12"}"#)
        #expect(post.headers["x-agency-token"] == "w-1")
    }

    @Test("stopping a push token is a write with no body, to the token's own path; a refusal is an error")
    func unregisterPushToken() async throws {
        let transport = ScriptedTransport([("GET /api/session", json(session)), ("DELETE /api/push/tokens/ab12", json("", status: 204))])
        try await client(transport).unregisterPushToken("ab12")
        let delete = try #require(transport.requests.last)
        #expect(delete.method == "DELETE" && delete.path == "/api/push/tokens/ab12")
        #expect(delete.body == nil)
        #expect(delete.headers["x-agency-token"] == "w-1")
        #expect(delete.headers[NodeClient.accessHeader] == "access-jwt")

        let refused = ScriptedTransport([("GET /api/session", json(session)), ("DELETE /api/push/tokens/ab12", json(#"{"error":"no"}"#, status: 500))])
        await #expect(throws: NodeError.self) { try await client(refused).unregisterPushToken("ab12") }
        await #expect(throws: NodeError.notSignedIn) { try await client(ScriptedTransport([]), token: nil).unregisterPushToken("ab12") }
    }

    @Test("a token that would add a path segment is escaped when it is stopped")
    func unregisterEscapes() async throws {
        let transport = ScriptedTransport([("GET /api/session", json(session)), ("DELETE /api/push/tokens/a%2Fb", json("", status: 204))])
        try await client(transport).unregisterPushToken("a/b")
        #expect(transport.requests.last?.path == "/api/push/tokens/a%2Fb")
    }

    @Test("the kinds a push token wants are sent as `events`, an empty list included, and left out when not given")
    func pushTokenEvents() async throws {
        for (events, expected) in [
            (["request", "device"], #"{"environment":"sandbox","events":["request","device"],"label":"iPhone","token":"ab12"}"#),
            ([], #"{"environment":"sandbox","events":[],"label":"iPhone","token":"ab12"}"#),
        ] as [([String], String)] {
            let transport = ScriptedTransport([("GET /api/session", json(session)), ("POST /api/push/tokens", fixture("push-token"))])
            _ = try await client(transport).registerPushToken(token: "ab12", environment: "sandbox", label: "iPhone", events: events)
            #expect(String(decoding: try #require(transport.requests.last?.body), as: UTF8.self) == expected)
        }
    }

    @Test("a node that predates kinds answers a token without `events`")
    func pushTokenWithoutEvents() throws {
        let text = #"{"environment":"production","label":"iPhone","registeredAt":"2026-11-24T12:00:00Z","token":"ab12"}"#
        #expect(try JSONDecoder().decode(RegisteredPushToken.self, from: Data(text.utf8)).events == nil)
    }

    @Test("an id from the node never adds a segment or a query to the path")
    func segments() async throws {
        let transport = ScriptedTransport([("GET /api/orgs/a%2Fb/teams/c%3Fd%23e/view", fixture("team-snapshot-local"))])
        _ = try await client(transport).teamSnapshot(org: "a/b", team: "c?d#e")
        #expect(transport.requests.first?.path == "/api/orgs/a%2Fb/teams/c%3Fd%23e/view")
        #expect(NodeClient.segment("acme-1.x_y~z") == "acme-1.x_y~z")
        #expect(NodeClient.segment("日本") == "%E6%97%A5%E6%9C%AC")
    }

    @Test("a 403 for another reason is the answer and is not sent again", arguments: [
        #"{"error":"writes are for the owner"}"#, #"{"error":"bad signature"}"#, #"{}"#, "",
    ])
    func otherRefusal(body: String) async {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", json(body, status: 403)),
        ])
        await #expect(throws: NodeError.self) { try await client(transport).submit(decision, org: "acme", team: "app") }
        #expect(transport.requests.map(\.method) == ["GET", "POST"])
    }

    @Test("the stale-token words under another status are not a reason to send again")
    func staleWordsElsewhere() async {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", json(#"{"error":"missing or wrong write token"}"#, status: 401)),
        ])
        await #expect(throws: NodeError.http(status: 401, message: "missing or wrong write token")) {
            try await client(transport).submit(decision, org: "acme", team: "app")
        }
        #expect(transport.requests.count == 2)
    }

    @Test("a second 403 is the answer; the client does not loop")
    func refusedTwice() async {
        let refusal = json(#"{"error":"missing or wrong write token"}"#, status: 403)
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", refusal),
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", refusal),
        ])
        await #expect(throws: NodeError.http(status: 403, message: "missing or wrong write token")) {
            try await client(transport).submit(decision, org: "acme", team: "app")
        }
        #expect(transport.requests.count == 4)
    }

    @Test("another refusal is not retried")
    func refusedByTeam() async {
        let transport = ScriptedTransport([
            ("GET /api/session", json(session)),
            ("POST /api/orgs/acme/teams/app/envelopes", json(#"{"error":"refused"}"#, status: 409)),
        ])
        await #expect(throws: NodeError.http(status: 409, message: "refused")) {
            try await client(transport).submit(decision, org: "acme", team: "app")
        }
        #expect(transport.requests.count == 2)
    }

    @Test("the node's clock is kept as an offset from the phone's")
    func clockOffset() async throws {
        let transport = ScriptedTransport([("GET /api/session", json(session))])
        let node = client(transport)
        #expect(await node.nodeNow() == phone)
        try await node.refreshSession()
        // The node says 2026-11-24T12:00:00Z; the phone's clock is 10 minutes slow.
        #expect(await node.nodeNow() == Date(timeIntervalSince1970: 1_795_521_600))
    }
}

@Suite("NodeClient: the sign-in exchange")
struct NodeClientRedeemTests {
    private let code = "c0de".padding(toLength: 43, withPad: "A", startingAt: 0)
    private let verifier = "v3r1f1er".padding(toLength: 43, withPad: "B", startingAt: 0)

    private func client(_ transport: ScriptedTransport) -> NodeClient {
        NodeClient(transport: transport, accessToken: { nil })
    }

    @Test("redeeming sends the code and the verifier, canonical, with no Access token and no write token, and asks for no session")
    func sends() async throws {
        let transport = ScriptedTransport([("POST /app/redeem", json(#"{"token":"a.b.c"}"#))])
        let token = try await client(transport).redeemSignIn(code: code, verifier: verifier)
        #expect(token == "a.b.c")
        let request = try #require(transport.requests.first)
        #expect(transport.requests.count == 1, "no GET /api/session first: there is no Access session to ask with")
        #expect(String(decoding: try #require(request.body), as: UTF8.self) == #"{"code":"\#(code)","verifier":"\#(verifier)"}"#)
        #expect(request.headers["content-type"] == "application/json")
        #expect(request.headers["cf-access-token"] == nil)
        #expect(request.headers["x-agency-token"] == nil)
    }

    private let ownerSession = #"{"writeToken":"w-1","now":"2026-11-24T12:00:00Z"}"#
    private func owner(_ transport: ScriptedTransport) -> NodeClient {
        NodeClient(transport: transport, accessToken: { "access-jwt" }, clock: { Date(timeIntervalSince1970: 1_795_521_000) })
    }

    @Test("entering a number is a write with the write token and plain JSON: the value is a decimal, which canonical JSON has no place for")
    func enterNumber() async throws {
        let answer = #"{"kpi":"revenue","date":"2026-11-20","value":1350000.5,"source":{"kind":"manual"},"enteredAt":"2026-11-24T12:00:00Z","enteredBy":"owner"}"#
        let transport = ScriptedTransport([("GET /api/session", json(ownerSession)), ("POST /api/orgs/acme/outcomes/observations", json(answer))])
        let saved = try await owner(transport).enterNumber(org: "acme", input: ObservationInput(kpi: "revenue", date: "2026-11-20", value: 1_350_000.5, note: "x"))
        #expect(saved.value == 1_350_000.5)
        let post = try #require(transport.requests.last)
        #expect(post.headers["x-agency-token"] == "w-1" && post.headers["content-type"] == "application/json")
        let data = try #require(post.body)
        let object = try JSONSerialization.jsonObject(with: data)
        let body = try #require(object as? [String: Any])
        #expect(body["kpi"] as? String == "revenue" && body["date"] as? String == "2026-11-20" && body["value"] as? Double == 1_350_000.5 && body["note"] as? String == "x")
        #expect(Set(body.keys) == ["kpi", "date", "value", "note"], "no field for who or when: the node sets them")
    }

    @Test("a number with no note sends none, a decimal stays a decimal, and a stale write token is renewed once")
    func enterNumberShape() async throws {
        let answer = #"{"kpi":"k","date":"2026-11-20","value":0.1,"source":{"kind":"manual"},"enteredAt":"2026-11-24T12:00:00Z","enteredBy":"owner"}"#
        let transport = ScriptedTransport([
            ("GET /api/session", json(ownerSession)),
            ("POST /api/orgs/acme/outcomes/observations", json(#"{"error":"missing or wrong write token"}"#, status: 403)),
            ("GET /api/session", json(#"{"writeToken":"w-2","now":"2026-11-24T12:00:00Z"}"#)),
            ("POST /api/orgs/acme/outcomes/observations", json(answer)),
        ])
        _ = try await owner(transport).enterNumber(org: "acme", input: ObservationInput(kpi: "k", date: "2026-11-20", value: 0.1))
        let post = try #require(transport.requests.last)
        let sent = try #require(post.body)
        let text = String(decoding: sent, as: UTF8.self)
        #expect(!text.contains("note") && text.contains("0.1"))
        #expect(post.headers["x-agency-token"] == "w-2")
    }

    @Test("the history is a read of the KPI's own path, newest first as the node gives it, and an id that would add a segment is escaped")
    func numberHistory() async throws {
        let answer = #"[{"kpi":"revenue","date":"2026-11-20","value":2,"source":{"kind":"connector","name":"export"},"enteredAt":"2026-11-22T09:00:00Z","enteredBy":"owner","note":"n"}]"#
        let transport = ScriptedTransport([("GET /api/orgs/acme/outcomes/observations?kpi=revenue&limit=100", json(answer))])
        let list = try await owner(transport).numberHistory(org: "acme", kpi: "revenue")
        #expect(list.map(\.value) == [2] && list[0].source == .connector(name: "export"))
        let escaped = ScriptedTransport([("GET /api/orgs/acme/outcomes/observations?kpi=a%2Fb&limit=5", json("[]"))])
        _ = try await owner(escaped).numberHistory(org: "acme", kpi: "a/b", limit: 5)
        #expect(escaped.requests.last?.path == "/api/orgs/acme/outcomes/observations?kpi=a%2Fb&limit=5")
    }

    @Test("the owner's push tokens are read from the list, each with when APNs last took a notice and when it last did not, when it has")
    func pushTokens() async throws {
        let answer = #"{"tokens":[{"token":"ab12cd34","environment":"production","label":"iPhone","events":["request","device"],"registeredAt":"2026-11-20T09:00:00Z","lastSucceededAt":"2026-11-26T08:00:00Z","lastFailure":{"at":"2026-11-26T09:00:00Z","reason":"APNs answered 503"}},{"token":"11112222","environment":"sandbox","label":"Mac","registeredAt":"2026-11-20T09:00:00Z"}]}"#
        let tokens = try await owner(ScriptedTransport([("GET /api/push/tokens", json(answer))])).pushTokens()
        #expect(tokens.map(\.token) == ["ab12cd34", "11112222"])
        #expect(tokens[0].lastSucceededAt?.text == "2026-11-26T08:00:00Z" && tokens[0].lastFailure?.reason == "APNs answered 503")
        #expect(tokens[1].lastSucceededAt == nil && tokens[1].lastFailure == nil, "left out until there is something to say")
    }

    @Test("a refusal's own words are shown without hidden characters, since an alert reads them, and the line breaks stay")
    func refusalText() async throws {
        let transport = ScriptedTransport([("GET /api/me", json(#"{"error":"no\u202E label\nsecond\u200B"}"#, status: 403))])
        do {
            _ = try await owner(transport).me()
            Issue.record("should be refused")
        } catch let error as NodeError {
            #expect(error == .http(status: 403, message: "no? label\nsecond?"))
        }
    }

    @Test("the redeem carries the app's identity too, though the sign-in has no Access token yet")
    func redeemIdentity() async throws {
        let transport = ScriptedTransport([("POST /app/redeem", json(#"{"token":"a.b.c"}"#))])
        let client = NodeClient(transport: transport, accessToken: { nil }, clientIdentity: "ios/0.3.1 (1)")
        _ = try await client.redeemSignIn(code: code, verifier: verifier)
        let request = try #require(transport.requests.first)
        #expect(request.headers["x-agency-client"] == "ios/0.3.1 (1)")
        #expect(request.headers["cf-access-token"] == nil)
    }

    @Test("a refusal is the node's one answer, a limit is 429, and a redirect says the Access policy is in the way")
    func refusals() async throws {
        let refused = ScriptedTransport([("POST /app/redeem", json(#"{"error":"the code is not valid"}"#, status: 400))])
        await #expect(throws: NodeError.http(status: 400, message: "the code is not valid")) {
            try await client(refused).redeemSignIn(code: code, verifier: verifier)
        }
        let limited = ScriptedTransport([("POST /app/redeem", json(#"{"error":"too many tries"}"#, status: 429))])
        await #expect(throws: NodeError.http(status: 429, message: "too many tries")) {
            try await client(limited).redeemSignIn(code: code, verifier: verifier)
        }
        let redirected = ScriptedTransport([("POST /app/redeem", HTTPResponse(status: 302, body: Data()))])
        do {
            _ = try await client(redirected).redeemSignIn(code: code, verifier: verifier)
            Issue.record("a redirect is not a token")
        } catch let NodeError.http(status, message) {
            #expect(status == 302)
            #expect(message?.contains("Access policy") == true)
        }
    }

    @Test("an answer with no token is unreadable, not an empty token")
    func unreadable() async {
        let transport = ScriptedTransport([("POST /app/redeem", json(#"{"other":"x"}"#))])
        await #expect(throws: NodeError.self) { try await client(transport).redeemSignIn(code: code, verifier: verifier) }
    }
}

@Suite("Me: the owner's other home nodes")
struct MeHomeNodesTests {
    private func me(_ extra: String) throws -> Me {
        let json = #"{"account":{"id":"alice","label":"A","fingerprint":"f"},"funding":[],"orgs":[]\#(extra)}"#
        return try JSONDecoder().decode(Me.self, from: Data(json.utf8))
    }

    @Test("the id the node says it uses in its pushes is read when it is there, and none when it is not")
    func nodeId() throws {
        #expect(try me(#","nodeId":"02ae509c6b20999646d3c600687ee1bf""#).nodeId == "02ae509c6b20999646d3c600687ee1bf")
        #expect(try me("").nodeId == nil)
    }

    @Test("the node's answer lists them in its order, from the node's own fixture")
    func fixtureAnswer() throws {
        let me = try JSONDecoder().decode(Me.self, from: Fixtures.data("me-home-nodes"))
        #expect(me.homeNodes == [HomeNodeRef(label: "Work", url: "https://work.example.com"), HomeNodeRef(label: "Lab", url: "http://127.0.0.1:4500")])
    }

    @Test("a node that names none, or does not tell this viewer, gives an empty list")
    func none() throws {
        #expect(try me("").homeNodes.isEmpty)
        #expect(try me(#","homeNodes":[]"#).homeNodes.isEmpty)
    }

    @Test("an entry without its address is an error, not a node with no address")
    func broken() {
        #expect(throws: DecodingError.self) { try me(#","homeNodes":[{"label":"Work"}]"#) }
    }
}
