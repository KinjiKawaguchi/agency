import Foundation
import Testing
@testable import AgencyKit

@Suite("PushRegistration")
struct PushRegistrationTests {
    @Test("a device token is written as lowercase hex, two digits per byte")
    func hex() throws {
        let token = try #require(PushToken(deviceToken: Data([0x00, 0x0a, 0xff, 0x10, 0x7b])))
        #expect(token.hex == "000aff107b")
    }

    @Test("an empty device token is refused")
    func emptyToken() {
        #expect(PushToken(deviceToken: Data()) == nil)
    }

    @Test("the aps-environment entitlement maps to the node's environment names")
    func environments() {
        #expect(PushEnvironment(apsEnvironment: "development") == .sandbox)
        #expect(PushEnvironment(apsEnvironment: "production") == .production)
        #expect(PushEnvironment(apsEnvironment: "") == nil)
        #expect(PushEnvironment(apsEnvironment: "$(APS_ENVIRONMENT)") == nil)
    }

    @Test("a registration encodes to the body the node's route takes")
    func encodesForTheNode() throws {
        let token = try #require(PushToken(deviceToken: Data([0xab, 0xcd])))
        let registration = PushRegistration(token: token, environment: .sandbox, label: "iPhone")
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let json = String(decoding: try encoder.encode(registration), as: UTF8.self)
        #expect(json == #"{"environment":"sandbox","label":"iPhone","token":"abcd"}"#)
    }
}
