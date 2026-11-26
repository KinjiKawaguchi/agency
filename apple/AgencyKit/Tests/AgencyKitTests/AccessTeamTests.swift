import Foundation
import Testing
@testable import AgencyKit

@Suite("AccessTeam")
struct AccessTeamTests {
    @Test("the team host comes from the node's redirect to its Access login page")
    func fromRedirect() {
        #expect(AccessTeam.host(fromRedirect: "https://acme.cloudflareaccess.com/cdn-cgi/access/login/node.example.com?kid=1&redirect_url=%2F") == "acme.cloudflareaccess.com")
        #expect(AccessTeam.host(fromRedirect: "https://ACME.CloudflareAccess.com:443/cdn-cgi/access/login/node.example.com") == "acme.cloudflareaccess.com")
        #expect(AccessTeam.host(fromRedirect: "https://team-1.cloudflareaccess.com/cdn-cgi/access/login/x") == "team-1.cloudflareaccess.com")
    }

    @Test("a redirect that is not to one team's Access login is not a team", arguments: [
        "", "/cdn-cgi/access/login/node.example.com", "http://acme.cloudflareaccess.com/cdn-cgi/access/login/x",
        "https://acme.cloudflareaccess.com:8443/cdn-cgi/access/login/x",
        "https://acme.cloudflareaccess.com/other", "https://acme.cloudflareaccess.com/",
        "https://cloudflareaccess.com/cdn-cgi/access/login/x", "https://evil.example/cdn-cgi/access/login/x",
        "https://acme.cloudflareaccess.com.evil.example/cdn-cgi/access/login/x",
        "https://acme.cloudflareaccess.com@evil.example/cdn-cgi/access/login/x",
        "https://user@acme.cloudflareaccess.com/cdn-cgi/access/login/x",
        "https://user:secret@acme.cloudflareaccess.com/cdn-cgi/access/login/x",
        "https://:secret@acme.cloudflareaccess.com/cdn-cgi/access/login/x",
        "https://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/cdn-cgi/access/login/x",
        "https://a.b.cloudflareaccess.com/cdn-cgi/access/login/x", "https://-a.cloudflareaccess.com/cdn-cgi/access/login/x",
    ])
    func notATeam(location: String) {
        #expect(AccessTeam.host(fromRedirect: location) == nil)
    }

    @Test("a team host is one plain label before the Access domain")
    func teamHosts() {
        #expect(AccessTeam.isTeamHost("acme.cloudflareaccess.com"))
        #expect(AccessTeam.isTeamHost("a-b.cloudflareaccess.com"))
        #expect(!AccessTeam.isTeamHost("cloudflareaccess.com"))
        #expect(!AccessTeam.isTeamHost("x.y.cloudflareaccess.com"))
        #expect(!AccessTeam.isTeamHost("ACME.cloudflareaccess.com"))
        #expect(!AccessTeam.isTeamHost("a_b.cloudflareaccess.com"))
        // Long enough that cutting the suffix's length off leaves a plain label: the suffix must be checked.
        #expect(!AccessTeam.isTeamHost("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))
    }
}
