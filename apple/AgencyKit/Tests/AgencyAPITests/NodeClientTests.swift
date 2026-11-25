import AgencyProtocol
import Foundation
import Testing
@testable import AgencyAPI

/// A transport that answers from a script and remembers what it was asked.
private final class ScriptedTransport: HTTPTransport, @unchecked Sendable {
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

private func json(_ text: String, status: Int = 200) -> HTTPResponse { HTTPResponse(status: status, body: Data(text.utf8)) }
private func fixture(_ name: String, status: Int = 200) -> HTTPResponse { HTTPResponse(status: status, body: try! Fixtures.data(name)) }

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
