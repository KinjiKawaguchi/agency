import Foundation
import Testing
@testable import AgencyKit

#if canImport(JavaScriptCore)
import JavaScriptCore

/// Runs the script in JavaScriptCore against a stand-in for the page's `window`.
private struct FakePage {
    let context = JavaScript.makeContext()
    let events: JSValue

    init(origin: String) {
        context.evaluateScript("""
        var events = [];
        var posted = [];
        var window = { location: { origin: \(origin.debugDescription) } };
        window.webkit = { messageHandlers: { agency: { postMessage: function (message) { posted.push(message); } } } };
        window.dispatchEvent = function (event) { events.push({ type: event.type, detail: event.detail }); };
        function CustomEvent(type, init) { this.type = type; this.detail = init.detail; }
        """)
        events = context.objectForKeyedSubscript("events")
    }

    func run(_ script: String) { context.evaluateScript(script) }

    /// What the page has handed to the app's message handler, as JSON.
    var postedMessages: String { context.evaluateScript("JSON.stringify(posted)").toString() }

    var native: JSValue { context.evaluateScript("window.agencyNative") }
    var eventCount: Int { Int(events.objectForKeyedSubscript("length").toInt32()) }
}

@Suite("NativeBridge")
struct NativeBridgeTests {
    private let homeNode = try! HomeNodeURL(validating: "https://node.example.com")
    private let registration = PushRegistration(
        token: PushToken(deviceToken: Data([0xab, 0xcd]))!, environment: .production, label: "iPhone"
    )

    @Test("on the home node the page sees the platform and the registration")
    func exposesTheRegistration() {
        let page = FakePage(origin: "https://node.example.com")
        page.run(NativeBridge.script(homeNode: homeNode, push: registration, announcesChange: false))
        #expect(page.native.forProperty("platform").toString() == "ios")
        let push = page.native.forProperty("push")
        #expect(push?.forProperty("token").toString() == "abcd")
        #expect(push?.forProperty("environment").toString() == "production")
        #expect(push?.forProperty("label").toString() == "iPhone")
        #expect(page.eventCount == 0)
    }

    @Test("without a registration, push is null")
    func nullBeforeRegistration() {
        let page = FakePage(origin: "https://node.example.com")
        page.run(NativeBridge.script(homeNode: homeNode, push: nil, announcesChange: false))
        #expect(page.native.forProperty("platform").toString() == "ios")
        #expect(page.native.forProperty("push").isNull)
    }

    @Test("a page that is already open is told through an event")
    func announcesChange() {
        let page = FakePage(origin: "https://node.example.com")
        page.run(NativeBridge.script(homeNode: homeNode, push: registration, announcesChange: true))
        #expect(page.eventCount == 1)
        let event = page.events.atIndex(0)
        #expect(event?.forProperty("type").toString() == "agency:native-push")
        #expect(event?.forProperty("detail").forProperty("token").toString() == "abcd")
    }

    @Test("setBadge hands the number to the app's message handler")
    func setBadge() {
        let page = FakePage(origin: "https://node.example.com")
        page.run(NativeBridge.script(homeNode: homeNode, push: nil, announcesChange: false))
        page.run("window.agencyNative.setBadge(3); window.agencyNative.setBadge(0);")
        #expect(page.postedMessages == #"[{"type":"badge","count":3},{"type":"badge","count":0}]"#)
    }

    @Test("setBadge still works after the state was replaced by an update")
    func setBadgeAfterUpdate() {
        let page = FakePage(origin: "https://node.example.com")
        page.run(NativeBridge.script(homeNode: homeNode, push: nil, announcesChange: false))
        page.run(NativeBridge.script(homeNode: homeNode, push: registration, announcesChange: true))
        page.run("window.agencyNative.setBadge(5);")
        #expect(page.postedMessages == #"[{"type":"badge","count":5}]"#)
    }

    @Test("setBadge does nothing, and does not throw, where there is no message handler")
    func setBadgeWithoutHandler() {
        let page = FakePage(origin: "https://node.example.com")
        page.run("delete window.webkit;")
        page.run(NativeBridge.script(homeNode: homeNode, push: nil, announcesChange: false))
        page.run("window.agencyNative.setBadge(3);")
        #expect(page.postedMessages == "[]")
        #expect(page.context.exception == nil)
    }

    @Test("pages of any other origin get nothing", arguments: [
        "https://acme.cloudflareaccess.com",
        "https://node.example.com:8443",
        "http://node.example.com",
        "https://evil.example",
        "null",
    ])
    func ignoresOtherOrigins(origin: String) {
        let page = FakePage(origin: origin)
        page.run(NativeBridge.script(homeNode: homeNode, push: registration, announcesChange: true))
        #expect(page.native.isUndefined)
        #expect(page.eventCount == 0)
    }

    @Test("an origin with a port is matched as a browser reports it")
    func originWithPort() throws {
        let local = try HomeNodeURL(validating: "http://localhost:3000")
        let page = FakePage(origin: "http://localhost:3000")
        page.run(NativeBridge.script(homeNode: local, push: nil, announcesChange: false))
        #expect(page.native.forProperty("platform").toString() == "ios")
    }
}
#endif

@Suite("NativeBridge origin")
struct NativeBridgeOriginTests {
    @Test("the origin leaves out a default port and keeps any other")
    func origin() throws {
        #expect(try HomeNodeURL(validating: "node.example.com").origin == "https://node.example.com")
        #expect(try HomeNodeURL(validating: "https://node.example.com:443").origin == "https://node.example.com")
        #expect(try HomeNodeURL(validating: "https://node.example.com:8443").origin == "https://node.example.com:8443")
        #expect(try HomeNodeURL(validating: "http://studio.local").origin == "http://studio.local")
        #expect(try HomeNodeURL(validating: "http://localhost:3000").origin == "http://localhost:3000")
    }

    @Test("same-origin means the same scheme, host and effective port")
    func sameOrigin() throws {
        let node = try HomeNodeURL(validating: "https://node.example.com")
        #expect(node.hasSameOrigin(URL(string: "https://NODE.example.com:443/desk?item=1")!))
        #expect(!node.hasSameOrigin(URL(string: "http://node.example.com/")!))
        #expect(!node.hasSameOrigin(URL(string: "https://node.example.com:8443/")!))
        #expect(!node.hasSameOrigin(URL(string: "https://sub.node.example.com/")!))
        #expect(!node.hasSameOrigin(URL(string: "mailto:alice@example.com")!))
    }

    @Test("an origin in parts, as WebKit reports a frame, treats port 0 as the default")
    func originInParts() throws {
        let node = try HomeNodeURL(validating: "https://node.example.com")
        #expect(node.hasSameOrigin(scheme: "https", host: "node.example.com", port: 0))
        #expect(node.hasSameOrigin(scheme: "HTTPS", host: "Node.Example.com", port: 443))
        #expect(!node.hasSameOrigin(scheme: "http", host: "node.example.com", port: 0))
        #expect(!node.hasSameOrigin(scheme: "https", host: "evil.example", port: 0))
        #expect(!node.hasSameOrigin(scheme: "https", host: "node.example.com", port: 8443))
        let local = try HomeNodeURL(validating: "http://localhost:3000")
        #expect(local.hasSameOrigin(scheme: "http", host: "localhost", port: 3000))
        #expect(!local.hasSameOrigin(scheme: "http", host: "localhost", port: 0))
    }
}
