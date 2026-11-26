import AgencyKit
@testable import AgencyModel
import Foundation
import Testing

@Suite("SignInExchange")
struct SignInExchangeTests {
    /// Bytes 0...31 and 32...63: what a test generator hands out, state first.
    private static func bytes(from first: UInt8) -> [UInt8] { (0..<32).map { first + UInt8($0) } }

    private let state = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
    private let verifier = "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8"
    /// base64url(SHA-256(verifier)), computed by a second implementation (Python's hashlib), and what the node expects.
    private let challenge = "zwkx4Wi0nph1A8rxivb-JTtrPYKoEAjD6OHuZ8fI3FU"
    private let code = String(repeating: "c", count: 43)

    private func exchange(_ host: String, associated: [String] = ["node.example.com"], now: @escaping @Sendable () -> Date = { Date(timeIntervalSince1970: 1_795_521_600) }) throws -> SignInExchange {
        var next: UInt8 = 0
        return try #require(SignInExchange(
            homeNode: try HomeNodeURL(validating: host), associatedHosts: associated,
            randomBytes: { defer { next += 32 }; return Self.bytes(from: next) }, now: now
        ))
    }

    @Test("the state and the verifier are 256 random bits each, and the challenge is the SHA-256 of the verifier as the node wants it")
    func secrets() throws {
        let one = try exchange("node.example.com")
        #expect(one.state == state)
        #expect(one.verifier == verifier)
        #expect(one.challenge == challenge)
        let real = try #require(SignInExchange(homeNode: try HomeNodeURL(validating: "node.example.com"), associatedHosts: []))
        #expect(real.state.count == 43 && real.verifier.count == 43)
        #expect(real.state != real.verifier)
        #expect(real.verifier != one.verifier, "a new verifier for each sign-in")
    }

    @Test("a generator that does not give 32 bytes makes no sign-in")
    func badRandom() throws {
        let node = try HomeNodeURL(validating: "node.example.com")
        #expect(SignInExchange(homeNode: node, associatedHosts: [], randomBytes: { [1, 2, 3] }) == nil)
    }

    @Test("only https is signed in to: Access is on https, and a plain http node has nothing to sign in to")
    func onlyHttps() throws {
        #expect(SignInExchange(homeNode: try HomeNodeURL(validating: "http://localhost:4400"), associatedHosts: []) == nil)
    }

    @Test("the start URL carries state, challenge and the transport, and nothing else; the verifier is not in it")
    func startURL() throws {
        let link = try exchange("node.example.com")
        #expect(link.startURL.absoluteString == "https://node.example.com/app/handoff?state=\(state)&challenge=\(challenge)&via=link")
        let scheme = try exchange("other.example.org")
        #expect(scheme.startURL.absoluteString == "https://other.example.org/app/handoff?state=\(state)&challenge=\(challenge)&via=scheme")
        #expect(!scheme.startURL.absoluteString.contains(verifier))
        let ported = try exchange("https://node.example.net:8443")
        #expect(ported.startURL.absoluteString == "https://node.example.net:8443/app/handoff?state=\(state)&challenge=\(challenge)&via=scheme")
    }

    @Test("a host the build has an associated domain for gets the universal link, and any other the custom scheme", arguments: [
        ("node.example.com", ["node.example.com"], SignInExchange.Transport.universalLink),
        ("Node.Example.com", ["NODE.example.com"], .universalLink),
        ("node.example.com", [], .customScheme),
        ("node.example.com", ["other.example.com"], .customScheme),
        ("node.example.com", ["example.com"], .customScheme),
        ("a.example.com", ["*.example.com"], .universalLink),
        ("example.com", ["*.example.com"], .customScheme),
        ("a.b.example.com", ["*.example.com"], .customScheme),
        ("xexample.com", ["*.example.com"], .customScheme),
        ("a.example.org", ["*.example.com"], .customScheme),
        ("node.example.com", [""], .customScheme),
        ("node.example.com", ["other.example.net", "*.example.com"], .universalLink),
    ])
    func transport(host: String, associated: [String], expected: SignInExchange.Transport) throws {
        #expect(try exchange(host, associated: associated).transport == expected)
    }

    @Test("a universal link works only on port 443: another port has no association, so it takes the scheme")
    func port() throws {
        #expect(try exchange("https://node.example.com:8443", associated: ["node.example.com"]).transport == .customScheme)
    }

    @Test("a callback gives its code when it is this sign-in's: by link")
    func codeByLink() throws {
        let one = try exchange("node.example.com")
        #expect(try one.code(from: URL(string: "https://node.example.com/app/callback#code=\(code)&state=\(state)")!) == code)
        #expect(one.callbackHost == "node.example.com" && one.callbackScheme == nil)
    }

    @Test("a callback gives its code when it is this sign-in's: by scheme")
    func codeByScheme() throws {
        let one = try exchange("other.example.org")
        #expect(try one.code(from: URL(string: "agency-signin://callback#code=\(code)&state=\(state)")!) == code)
        #expect(one.callbackScheme == "agency-signin" && one.callbackHost == nil)
    }

    @Test("a callback that is not this sign-in's gives nothing", arguments: [
        "https://evil.example.com/app/callback#code=CODE&state=STATE",
        "https://node.example.com/app/other#code=CODE&state=STATE",
        "https://node.example.com:8443/app/callback#code=CODE&state=STATE",
        "http://node.example.com/app/callback#code=CODE&state=STATE",
        "agency-signin://callback#code=CODE&state=STATE",
        "https://node.example.com/app/callback?code=CODE&state=STATE",
        "https://node.example.com/app/callback",
        "https://node.example.com/app/callback#code=CODE",
        "https://node.example.com/app/callback#code=CODE&state=STATE&extra=1",
        "https://node.example.com/app/callback#code=CODE&code=CODE&state=STATE",
        "https://node.example.com/app/callback#code&state=STATE",
        "https://node.example.com/app/callback?x=1#code=CODE&state=STATE",
        "https://node.example.com/app/callback?#code=CODE&state=STATE",
    ])
    func rejectsLink(template: String) throws {
        let one = try exchange("node.example.com")
        let url = URL(string: template.replacingOccurrences(of: "CODE", with: code).replacingOccurrences(of: "STATE", with: state))!
        #expect(throws: SignInExchangeError.self) { try one.code(from: url) }
    }

    @Test("the scheme transport refuses a callback of another host, scheme, path or port", arguments: [
        "agency-signin://evil#code=CODE&state=STATE",
        "agency-signin://callback/extra#code=CODE&state=STATE",
        "agency-signin://callback:80#code=CODE&state=STATE",
        "agency-signin://user@callback#code=CODE&state=STATE",
        "other-scheme://callback#code=CODE&state=STATE",
        "https://other.example.org/app/callback#code=CODE&state=STATE",
        "agency-signin://callback#code=CODE",
        "agency-signin://callback?x=1#code=CODE&state=STATE",
    ])
    func rejectsScheme(template: String) throws {
        let one = try exchange("other.example.org")
        let url = URL(string: template.replacingOccurrences(of: "CODE", with: code).replacingOccurrences(of: "STATE", with: state))!
        #expect(throws: SignInExchangeError.self) { try one.code(from: url) }
    }

    @Test("another sign-in's state is a mismatch, and a code that is not 43 base64url characters is missing")
    func stateAndCode() throws {
        let one = try exchange("node.example.com")
        let other = String(repeating: "z", count: 43)
        #expect(throws: SignInExchangeError.stateMismatch) { try one.code(from: URL(string: "https://node.example.com/app/callback#code=\(code)&state=\(other)")!) }
        for bad in [String(code.dropLast()), code + "A", String(code.dropLast()) + "+", String(code.dropLast()) + "/", String(code.dropLast()) + "=", ""] {
            #expect(throws: SignInExchangeError.missingCode, "\(bad)") {
                try one.code(from: URL(string: "https://node.example.com/app/callback#code=\(bad)&state=\(state)")!)
            }
        }
    }

    // MARK: Once, and only for its own callback

    @Test("a callback delivered twice (the session's completion and the app's URL handler) gives its code once")
    func once() throws {
        let one = try exchange("node.example.com")
        let url = URL(string: "https://node.example.com/app/callback#code=\(code)&state=\(state)")!
        #expect(try one.code(from: url) == code)
        #expect(throws: SignInExchangeError.alreadyUsed) { try one.code(from: url) }
        #expect(throws: SignInExchangeError.alreadyUsed) { try one.code(from: url) }
    }

    @Test("a wrong callback does not use the sign-in up: a page that opens agency-signin://callback# cannot cancel it")
    func wrongDoesNotBurn() throws {
        let one = try exchange("other.example.org")
        for wrong in [
            "agency-signin://callback#code=\(code)&state=\(String(repeating: "z", count: 43))",
            "agency-signin://callback#code=short&state=\(state)",
            "agency-signin://callback#",
            "agency-signin://callback",
            "agency-signin://evil#code=\(code)&state=\(state)",
            "agency-signin://callback?x=1#code=\(code)&state=\(state)",
        ] {
            #expect(throws: SignInExchangeError.self, "\(wrong)") { try one.code(from: URL(string: wrong)!) }
        }
        #expect(try one.code(from: URL(string: "agency-signin://callback#code=\(code)&state=\(state)")!) == code, "the real callback still works")
    }

    @Test("a sign-in lives ten minutes: after that the callback is expired, and a late one gives nothing")
    func lifetime() throws {
        let clock = Clock(Date(timeIntervalSince1970: 1_795_521_600))
        let one = try exchange("node.example.com", now: { clock.now })
        let url = URL(string: "https://node.example.com/app/callback#code=\(code)&state=\(state)")!
        clock.advance(10 * 60)
        #expect(try one.code(from: url) == code, "at the ten minutes it still works")
        let late = try exchange("node.example.com", now: { clock.now })
        clock.advance(10 * 60 + 1)
        #expect(throws: SignInExchangeError.expired) { try late.code(from: url) }
    }

    @Test("a print, a log line or a dump says where it signs in and nothing secret")
    func hidesSecrets() throws {
        let one = try exchange("node.example.com")
        var dumped = ""
        dump(one, to: &dumped)
        for text in ["\(one)", String(describing: one), String(reflecting: one), one.description, one.debugDescription, dumped] {
            #expect(text.contains("node.example.com"))
            #expect(!text.contains(verifier) && !text.contains(state) && !text.contains(challenge), "\(text)")
        }
    }

    @Test("redeeming gives the verifier to the redeemer and to nobody else, and returns the token")
    func redeem() async throws {
        let one = try exchange("node.example.com")
        let redeemer = Redeemer(token: "a.b.c")
        #expect(try await one.redeem(code: code, using: redeemer) == "a.b.c")
        #expect(redeemer.calls == [Redeemer.Call(code: code, verifier: verifier)])
        #expect(!one.startURL.absoluteString.contains(verifier))
    }
}

private final class Clock: @unchecked Sendable {
    private let lock = NSLock()
    private var current: Date
    init(_ start: Date) { current = start }
    var now: Date { lock.withLock { current } }
    func advance(_ seconds: TimeInterval) { lock.withLock { current = current.addingTimeInterval(seconds) } }
}

private final class Redeemer: SignInRedeeming, @unchecked Sendable {
    struct Call: Equatable { let code: String; let verifier: String }
    private let lock = NSLock()
    private var recorded: [Call] = []
    private let token: String
    init(token: String) { self.token = token }
    var calls: [Call] { lock.withLock { recorded } }
    func redeemSignIn(code: String, verifier: String) async throws -> String {
        lock.withLock { recorded.append(Call(code: code, verifier: verifier)) }
        return token
    }
}
