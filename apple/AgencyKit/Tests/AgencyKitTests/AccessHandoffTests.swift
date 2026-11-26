import Foundation
import Testing
@testable import AgencyKit

@Suite("AccessHandoff")
struct AccessHandoffTests {
    private let node = try! HomeNodeURL(validating: "https://node.example.com")
    private let now = Date(timeIntervalSince1970: 1_800_000_000)
    private let bytes = [UInt8](0..<32)

    private func make(
        node: HomeNodeURL? = nil,
        associatedHost: String = "node.example.com",
        bytes: [UInt8]? = nil
    ) -> AccessHandoff? {
        let bytes = bytes ?? self.bytes
        return AccessHandoff(homeNode: node ?? self.node, associatedHost: associatedHost, randomBytes: { bytes })
    }

    private func token(exp: Any = 1_800_003_600, segments: Int = 3) -> String {
        let payload = try! JSONSerialization.data(withJSONObject: ["exp": exp, "email": "alice@example.com"])
        let parts = ["eyJhbGciOiJSUzI1NiJ9", Base64URL.encode([UInt8](payload)), "c2ln"]
        return parts.prefix(segments).joined(separator: ".")
    }

    private func callback(
        _ handoff: AccessHandoff,
        token: String? = nil,
        state: String? = nil,
        base: String = "https://node.example.com/app/callback"
    ) -> URL {
        URL(string: "\(base)#token=\(token ?? self.token())&state=\(state ?? handoff.state)")!
    }

    // MARK: Starting

    @Test("the state is 43 base64url characters made from 32 bytes")
    func state() throws {
        let handoff = try #require(make())
        #expect(handoff.state == "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8")
        #expect(handoff.state.count == 43)
    }

    @Test("bytes of the wrong count make no state", arguments: [0, 31, 33])
    func wrongByteCount(count: Int) {
        #expect(make(bytes: [UInt8](repeating: 7, count: count)) == nil)
    }

    @Test("the system's bytes are 32 and differ each time")
    func systemBytes() {
        #expect(AccessHandoff.systemRandomBytes().count == 32)
        #expect(AccessHandoff.systemRandomBytes() != AccessHandoff.systemRandomBytes())
    }

    @Test("only a node on https port 443 that is the associated host can hand off")
    func availability() {
        #expect(make() != nil)
        #expect(make(associatedHost: "NODE.example.com") != nil)
        #expect(make(associatedHost: "other.example.com") == nil)
        #expect(make(associatedHost: "") == nil)
        #expect(make(node: try! HomeNodeURL(validating: "https://node.example.com:8443")) == nil)
        #expect(make(node: try! HomeNodeURL(validating: "http://localhost")) == nil)
        #expect(make(node: try! HomeNodeURL(validating: "http://localhost:443"), associatedHost: "localhost") == nil)
        #expect(make(node: try! HomeNodeURL(validating: "http://node.local"), associatedHost: "node.local") == nil)
    }

    @Test("the session starts on the handoff route of the node with the state")
    func startURL() throws {
        let handoff = try #require(make())
        #expect(handoff.startURL.absoluteString == "https://node.example.com/app/handoff?state=\(handoff.state)")
        #expect(handoff.callbackHost == "node.example.com")
        #expect(handoff.callbackPath == "/app/callback")
    }

    // MARK: Receiving

    @Test("a good callback gives the token, with the host and the time it expires")
    func goodCallback() throws {
        let handoff = try #require(make())
        let jwt = token()
        let result = try handoff.token(from: callback(handoff, token: jwt), now: now)
        #expect(result.value == jwt)
        #expect(result.host == "node.example.com")
        #expect(result.expires == Date(timeIntervalSince1970: 1_800_003_600))
        #expect(AccessToken.headerName == "cf-access-token")
    }

    @Test("a callback of another host, scheme, port or path is not ours", arguments: [
        "https://other.example.com/app/callback",
        "https://node.example.com.evil.example/app/callback",
        "http://node.example.com/app/callback",
        "https://node.example.com:8443/app/callback",
        "https://node.example.com/app/callback/",
        "https://node.example.com/app/handoff",
        "https://node.example.com/",
        "agency://node.example.com/app/callback",
    ])
    func foreignCallback(base: String) throws {
        let handoff = try #require(make())
        #expect(throws: AccessHandoffError.notTheCallback) {
            try handoff.token(from: callback(handoff, base: base), now: now)
        }
    }

    @Test("the host is compared without case, and port 443 may be written")
    func hostCase() throws {
        let handoff = try #require(make())
        _ = try handoff.token(from: callback(handoff, base: "https://NODE.example.com:443/app/callback"), now: now)
    }

    @Test("a state of another sign-in is refused")
    func otherState() throws {
        let handoff = try #require(make())
        let other = try #require(self.make(bytes: [UInt8](repeating: 9, count: 32)))
        #expect(throws: AccessHandoffError.stateMismatch) {
            try handoff.token(from: callback(handoff, state: other.state), now: now)
        }
        #expect(throws: AccessHandoffError.stateMismatch) {
            try handoff.token(from: URL(string: "https://node.example.com/app/callback#token=\(token())")!, now: now)
        }
    }

    @Test("the token has to be in the fragment, never in the query")
    func tokenInQuery() throws {
        let handoff = try #require(make())
        let url = URL(string: "https://node.example.com/app/callback?token=\(token())&state=\(handoff.state)")!
        #expect(throws: AccessHandoffError.notTheCallback) { try handoff.token(from: url, now: now) }
    }

    @Test("a repeated field or a part without a value is refused", arguments: [
        "token=A.B.C&token=D.E.F&state=%@",
        "token=%@&state=%@&state=%@",
        "token&state=%@",
        "state=%@&",
    ])
    func malformedFragment(template: String) throws {
        let handoff = try #require(make())
        let fragment = template.replacingOccurrences(of: "%@", with: handoff.state)
        let url = URL(string: "https://node.example.com/app/callback#\(fragment)")!
        #expect(throws: AccessHandoffError.notTheCallback) { try handoff.token(from: url, now: now) }
    }

    @Test("a callback without a token is refused")
    func missingToken() throws {
        let handoff = try #require(make())
        let url = URL(string: "https://node.example.com/app/callback#state=\(handoff.state)")!
        #expect(throws: AccessHandoffError.missingToken) { try handoff.token(from: url, now: now) }
    }

    @Test("what is not a JWT with an exp is refused", arguments: [
        "", "abc", "a.b", "a.b.c.d", "..", "a..c", "a.b c.c", "a.b+c.c", "a.%7B%7D.c", "a.e30.c",
    ])
    func malformedToken(value: String) throws {
        let handoff = try #require(make())
        #expect(throws: AccessHandoffError.malformedToken) {
            try handoff.token(from: callback(handoff, token: value), now: now)
        }
    }

    @Test("an empty segment, or a character a header value must not carry, is refused", arguments: [
        ".PAYLOAD.c2ln", "eyJhbGciOiJSUzI1NiJ9.PAYLOAD.", "eyJhbGciOiJSUzI1NiJ9.PAYLOAD", "eyJhbGciOiJSUzI1NiJ9.PAYLOAD.c2ln.c2ln", "eyJh;bGci.PAYLOAD.c2ln", "eyJhbGciOiJSUzI1NiJ9.PAYLOAD.c2 ln",
        "eyJhbGciOiJSUzI1NiJ9.PAYLOAD.c2ln;Path=/", "eyJhbGciOiJSUzI1NiJ9.PAYLOAD.c2ln\n",
    ])
    func segmentsAndCharacters(template: String) throws {
        let handoff = try #require(make())
        let payload = Base64URL.encode([UInt8](#"{"exp":1800003600}"#.utf8))
        let value = template.replacingOccurrences(of: "PAYLOAD", with: payload)
        let encoded = value.replacingOccurrences(of: " ", with: "%20").replacingOccurrences(of: "\n", with: "%0A")
        let url = URL(string: "https://node.example.com/app/callback#token=\(encoded)&state=\(handoff.state)")!
        #expect(throws: AccessHandoffError.malformedToken) { try handoff.token(from: url, now: now) }
    }

    @Test("a percent-encoded separator does not smuggle a field in")
    func encodedSeparator() throws {
        let handoff = try #require(make())
        let url = URL(string: "https://node.example.com/app/callback#token=\(token())%26state%3D\(handoff.state)")!
        #expect(throws: AccessHandoffError.self) { try handoff.token(from: url, now: now) }
    }

    @Test("an exp that is not a number is refused", arguments: ["\"soon\"", "true", "null", "[1]"])
    func exoticExp(json: String) throws {
        let handoff = try #require(make())
        let payload = Base64URL.encode([UInt8](#"{"exp":\#(json)}"#.utf8))
        #expect(throws: AccessHandoffError.malformedToken) {
            try handoff.token(from: callback(handoff, token: "e30.\(payload).c2ln"), now: now)
        }
    }

    @Test("a token past its exp, or expiring this second, is refused")
    func expired() throws {
        let handoff = try #require(make())
        let past = token(exp: 1_799_999_999)
        let atNow = token(exp: 1_800_000_000)
        #expect(throws: AccessHandoffError.expired) { try handoff.token(from: callback(handoff, token: past), now: now) }
        #expect(throws: AccessHandoffError.expired) { try handoff.token(from: callback(handoff, token: atNow), now: now) }
        _ = try handoff.token(from: callback(handoff, token: token(exp: 1_800_000_001)), now: now)
    }

    @Test("a fractional exp keeps its fraction")
    func fractionalExp() throws {
        let handoff = try #require(make())
        let result = try handoff.token(from: callback(handoff, token: token(exp: 1_800_003_600.5)), now: now)
        #expect(result.expires == Date(timeIntervalSince1970: 1_800_003_600.5))
    }

    @Test("a token over the size limit is refused")
    func oversized() throws {
        let handoff = try #require(make())
        let padding = String(repeating: "a", count: AccessHandoff.maxTokenLength)
        let big = "e30.\(Base64URL.encode([UInt8](#"{"exp":1800003600}"#.utf8))).\(padding)"
        #expect(throws: AccessHandoffError.malformedToken) {
            try handoff.token(from: callback(handoff, token: big), now: now)
        }
    }

    // MARK: A token that a code exchange gave

    @Test("a redeemed token is kept under the host it was redeemed at, with its own expiry")
    func redeemed() throws {
        let jwt = token()
        let kept = try AccessHandoff.token(fromJWT: jwt, host: "node.example.com", now: now)
        #expect(kept.host == "node.example.com")
        #expect(kept.value == jwt)
        #expect(kept.expires == Date(timeIntervalSince1970: 1_800_003_600))
    }

    @Test("a redeemed token that has expired, or is not a token, is refused")
    func redeemedRefused() {
        #expect(throws: AccessHandoffError.expired) {
            try AccessHandoff.token(fromJWT: token(exp: 1_799_999_999), host: "node.example.com", now: now)
        }
        #expect(throws: AccessHandoffError.expired) {
            try AccessHandoff.token(fromJWT: token(exp: 1_800_000_000), host: "node.example.com", now: now)
        }
        #expect(throws: AccessHandoffError.malformedToken) {
            try AccessHandoff.token(fromJWT: token(segments: 2), host: "node.example.com", now: now)
        }
    }
}
