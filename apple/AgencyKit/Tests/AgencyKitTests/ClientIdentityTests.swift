import Testing
@testable import AgencyKit

@Suite("Client identity")
struct ClientIdentityTests {
    @Test("the header is the platform, the version and the build, as the node reads it")
    func exact() {
        #expect(ClientIdentity.value(platform: .ios, version: "0.3.1", build: "202611261830") == "ios/0.3.1 (202611261830)")
        #expect(ClientIdentity.value(platform: .macos, version: "1.0.0", build: "7") == "macos/1.0.0 (7)")
        #expect(ClientIdentity.header == "x-agency-client")
    }

    @Test("with no version the app says nothing, and with no build it says the version alone")
    func absent() {
        #expect(ClientIdentity.value(platform: .ios, version: nil, build: "202611261830") == nil)
        #expect(ClientIdentity.value(platform: .ios, version: nil, build: nil) == nil)
        #expect(ClientIdentity.value(platform: .ios, version: "", build: "1") == nil)
        #expect(ClientIdentity.value(platform: .ios, version: "0.3.1", build: nil) == "ios/0.3.1")
        #expect(ClientIdentity.value(platform: .ios, version: "0.3.1", build: "") == "ios/0.3.1")
    }

    @Test("the version is exactly three numbers of one to six digits: anything else leaves the header out, as the node would ignore it whole")
    func versionForm() {
        for good in ["0.3.1", "1.0.0", "123456.123456.123456", "0.0.0"] {
            #expect(ClientIdentity.value(platform: .ios, version: good, build: nil) == "ios/\(good)", Comment(rawValue: good))
        }
        for bad in ["1.0", "0.3.1-beta", "1.0.0+5", "1.0.0.0", "1..0", ".1.0", "1.0.", "1234567.0.0", "a.b.c", "1.0.0 ", "１.0.0", "1/0.0", "1.0.0\r\nx-other: 1", ""] {
            #expect(ClientIdentity.value(platform: .ios, version: bad, build: "1") == nil, Comment(rawValue: bad))
        }
    }

    @Test("the build is only digits that fit an unsigned 64-bit integer; any other build is left out and the version is kept")
    func buildForm() {
        #expect(ClientIdentity.value(platform: .ios, version: "0.3.1", build: "1") == "ios/0.3.1 (1)")
        #expect(ClientIdentity.value(platform: .ios, version: "0.3.1", build: "18446744073709551615") == "ios/0.3.1 (18446744073709551615)")
        for bad in ["1.2.3", "18446744073709551616", "+5", "-1", "12 34", "", "a", "１２"] {
            #expect(ClientIdentity.value(platform: .ios, version: "0.3.1", build: bad) == "ios/0.3.1", Comment(rawValue: bad))
        }
    }

    @Test("the longest header that can be made is under the node's 64 characters")
    func longest() throws {
        let value = try #require(ClientIdentity.value(platform: .macos, version: "999999.999999.999999", build: "18446744073709551615"))
        #expect(value.count <= 64)
    }

    @Test("this app's identity is the platform it was built for")
    func current() {
        let value = ClientIdentity.current()
        #if os(macOS)
        #expect(value == nil || value?.hasPrefix("macos/") == true)
        #else
        #expect(value == nil || value?.hasPrefix("ios/") == true)
        #endif
    }
}
