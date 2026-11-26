import Foundation
import Testing
@testable import AgencyKit

@Suite("NavigationPolicy")
struct NavigationPolicyTests {
    private let policy = NavigationPolicy(homeNode: try! HomeNodeURL(validating: "https://node.example.com"), accessTeamHost: "acme.cloudflareaccess.com")

    private func decision(_ address: String, policy: NavigationPolicy? = nil) -> NavigationDecision {
        (policy ?? self.policy).decide(for: URL(string: address)!)
    }

    @Test("pages of the home node stay in the app")
    func homeNodeStays() {
        #expect(decision("https://node.example.com/") == .allowInApp)
        #expect(decision("https://node.example.com/acme/app/requests/1?x=2") == .allowInApp)
        #expect(decision("https://NODE.example.com:443/") == .allowInApp)
    }

    @Test("Access login pages stay in the app")
    func accessLoginStays() {
        #expect(decision("https://acme.cloudflareaccess.com/cdn-cgi/access/login/node.example.com?kid=1") == .allowInApp)
        #expect(decision("https://acme.cloudflareaccess.com/cdn-cgi/access/callback?code=1") == .allowInApp)
    }

    @Test("the login page of another Access team goes to Safari, and so does every page of it", arguments: [
        "https://other.cloudflareaccess.com/cdn-cgi/access/login/node.example.com?kid=1",
        "https://other.cloudflareaccess.com/cdn-cgi/access/callback?code=1",
        "https://other.cloudflareaccess.com/",
        "https://sub.acme.cloudflareaccess.com/",
    ])
    func otherTeams(address: String) {
        #expect(decision(address) == .openExternally)
    }

    @Test("until the node's team is known no Access page stays in the app")
    func teamUnknown() throws {
        let unknown = NavigationPolicy(homeNode: try HomeNodeURL(validating: "https://node.example.com"))
        #expect(unknown.accessTeamHost == nil)
        #expect(decision("https://acme.cloudflareaccess.com/cdn-cgi/access/login/node.example.com", policy: unknown) == .openExternally)
        #expect(decision("https://node.example.com/x", policy: unknown) == .allowInApp)
    }

    @Test("a team host that is not one is ignored, and the case of a host does not matter")
    func teamHostForms() throws {
        let node = try HomeNodeURL(validating: "https://node.example.com")
        for bad in ["evil.example", "cloudflareaccess.com", ".cloudflareaccess.com", "a.b.cloudflareaccess.com", "-x.cloudflareaccess.com", ""] {
            #expect(NavigationPolicy(homeNode: node, accessTeamHost: bad).accessTeamHost == nil, "\(bad)")
        }
        let upper = NavigationPolicy(homeNode: node, accessTeamHost: "ACME.cloudflareaccess.com")
        #expect(upper.accessTeamHost == "acme.cloudflareaccess.com")
        #expect(decision("https://Acme.CloudflareAccess.com/cdn-cgi/access/login/x", policy: upper) == .allowInApp)
    }

    @Test("look-alikes of the Access domain go to Safari", arguments: [
        "https://cloudflareaccess.com/",
        "https://evilcloudflareaccess.com/",
        "https://acme.cloudflareaccess.com.evil.example/",
        "https://acme.cloudflareaccess.com@evil.example/",
        "https://acme.cloudflareaccess.com:8443/",
        "http://acme.cloudflareaccess.com/",
        "http://acme.cloudflareaccess.com:443/",
    ])
    func accessLookAlikes(address: String) {
        #expect(decision(address) == .openExternally)
    }

    @Test("look-alikes and neighbours of the home node go to Safari", arguments: [
        "https://other.example.com/",
        "https://node.example.com.evil.example/",
        "https://evilnode.example.com/",
        "https://sub.node.example.com/",
        "https://node.example.com:8443/",
        "http://node.example.com/",
    ])
    func homeNodeLookAlikes(address: String) {
        #expect(decision(address) == .openExternally)
    }

    @Test("a web address without a host does nothing")
    func hostless() {
        #expect(decision("https:///path") == .block)
    }

    @Test("the home node's port decides, not the host alone")
    func portMatters() throws {
        let local = NavigationPolicy(homeNode: try HomeNodeURL(validating: "http://localhost:3000"))
        #expect(decision("http://localhost:3000/x", policy: local) == .allowInApp)
        #expect(decision("http://localhost:3001/x", policy: local) == .openExternally)
        #expect(decision("https://localhost:3000/x", policy: local) == .openExternally)
    }

    @Test("mail and phone links go to the system; other schemes do nothing", arguments: [
        ("mailto:alice@example.com", NavigationDecision.openExternally),
        ("tel:+15550100", NavigationDecision.openExternally),
        ("javascript:alert(1)", NavigationDecision.block),
        ("file:///etc/passwd", NavigationDecision.block),
        ("data:text/html,hi", NavigationDecision.block),
        ("blob:https://node.example.com/1234", NavigationDecision.block),
        ("itms-services://?action=download-manifest", NavigationDecision.block),
        ("about:blank", NavigationDecision.allowInApp),
    ])
    func otherSchemes(address: String, expected: NavigationDecision) {
        #expect(decision(address) == expected)
    }
}

@Suite("NavigationPolicy Access login page")
struct AccessLoginPageTests {
    private let policy = NavigationPolicy(homeNode: try! HomeNodeURL(validating: "https://node.example.com"), accessTeamHost: "acme.cloudflareaccess.com")

    @Test("the login page of an Access team is recognized", arguments: [
        "https://acme.cloudflareaccess.com/cdn-cgi/access/login/node.example.com?kid=1&redirect_url=%2F",
        "https://ACME.cloudflareaccess.com:443/cdn-cgi/access/login/node.example.com",
    ])
    func loginPage(address: String) {
        #expect(policy.isAccessLoginPage(URL(string: address)!))
    }

    @Test("other pages are not", arguments: [
        "https://other.cloudflareaccess.com/cdn-cgi/access/login/node.example.com",
        "https://acme.cloudflareaccess.com/cdn-cgi/access/logout",
        "https://acme.cloudflareaccess.com/cdn-cgi/access/callback?code=1",
        "https://acme.cloudflareaccess.com/",
        "https://node.example.com/cdn-cgi/access/login/node.example.com",
        "https://evilcloudflareaccess.com/cdn-cgi/access/login/x",
        "https://acme.cloudflareaccess.com:8443/cdn-cgi/access/login/x",
        "http://acme.cloudflareaccess.com/cdn-cgi/access/login/x",
        "https://acme.cloudflareaccess.com.evil.example/cdn-cgi/access/login/x",
    ])
    func otherPages(address: String) {
        #expect(!policy.isAccessLoginPage(URL(string: address)!))
    }
}
