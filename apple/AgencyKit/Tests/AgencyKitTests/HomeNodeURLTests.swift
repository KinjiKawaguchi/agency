import Foundation
import Testing
@testable import AgencyKit

@Suite("HomeNodeURL")
struct HomeNodeURLTests {
    @Test("a bare host name becomes an https origin")
    func bareHost() throws {
        let node = try HomeNodeURL(validating: "node.example.com")
        #expect(node.url.absoluteString == "https://node.example.com/")
        #expect(node.scheme == "https")
        #expect(node.host == "node.example.com")
        #expect(node.port == 443)
    }

    @Test("surrounding whitespace and letter case are ignored")
    func normalizes() throws {
        let node = try HomeNodeURL(validating: "  HTTPS://Node.Example.COM \n")
        #expect(node.url.absoluteString == "https://node.example.com/")
    }

    @Test("path, query and fragment are dropped")
    func dropsEverythingButTheOrigin() throws {
        let node = try HomeNodeURL(validating: "https://node.example.com/acme/app?x=1#top")
        #expect(node.url.absoluteString == "https://node.example.com/")
    }

    @Test("an explicit port is kept")
    func keepsPort() throws {
        let node = try HomeNodeURL(validating: "https://node.example.com:8443")
        #expect(node.url.absoluteString == "https://node.example.com:8443/")
        #expect(node.port == 8443)
    }

    @Test("plain http is accepted for this machine and .local names")
    func httpForLocalHosts() throws {
        #expect(try HomeNodeURL(validating: "http://localhost:3000").url.absoluteString == "http://localhost:3000/")
        #expect(try HomeNodeURL(validating: "http://127.0.0.1:3000").port == 3000)
        #expect(try HomeNodeURL(validating: "http://studio.local").port == 80)
    }

    @Test("plain http to any other host is rejected")
    func httpToRemoteHost() {
        #expect(throws: HomeNodeURLError.insecureRemoteHost) { try HomeNodeURL(validating: "http://node.example.com") }
    }

    @Test("input that cannot be an address is rejected", arguments: [
        ("", HomeNodeURLError.empty),
        ("   ", HomeNodeURLError.empty),
        ("ftp://node.example.com", HomeNodeURLError.unsupportedScheme),
        ("javascript://node.example.com", HomeNodeURLError.unsupportedScheme),
        ("https://alice:secret@node.example.com", HomeNodeURLError.hasCredentials),
        ("https://", HomeNodeURLError.invalid),
        ("https://node example.com", HomeNodeURLError.invalid),
        ("https://node..example.com", HomeNodeURLError.invalid),
        ("https://node.example.com.", HomeNodeURLError.invalid),
        ("https://-node.example.com", HomeNodeURLError.invalid),
        ("https://node_1.example.com", HomeNodeURLError.invalid),
        ("https://node.example.com:0", HomeNodeURLError.invalid),
        ("https://node.example.com:99999", HomeNodeURLError.invalid),
    ])
    func rejects(input: String, expected: HomeNodeURLError) {
        #expect(throws: expected) { try HomeNodeURL(validating: input) }
    }
}
