import Foundation
import Testing
@testable import AgencyAPI

@Suite("Devices on the wire")
struct DeviceWireTests {
    @Test("the device list: devices per org and the requests that wait")
    func deviceList() throws {
        let list = try JSONDecoder().decode(DeviceList.self, from: Fixtures.data("device-list"))
        #expect(list.orgs.count == 1)
        #expect(list.orgs[0].org == "acme")
        #expect(list.orgs[0].devices.map(\.id) == ["d-dbc298251c51"])
        #expect(list.orgs[0].devices[0].label == "Laptop")
        #expect(list.requests.map(\.pairingCode) == ["506e-f187-9d74"])
        #expect(list.requests[0].deviceId == "d-506ef1879d74")
    }

    @Test("a device request")
    func deviceRequest() throws {
        let request = try JSONDecoder().decode(DeviceRequest.self, from: Fixtures.data("device-request"))
        #expect(request.label == "iPhone")
        #expect(request.pairingCode == "506e-f187-9d74")
        #expect(request.requestedAt.text == "2026-11-24T12:00:00Z")
    }

    @Test("a renamed required field fails", arguments: ["pairingCode", "deviceId", "publicKey", "requests"])
    func required(field: String) throws {
        let text = try Fixtures.text("device-list").replacingOccurrences(of: "\"\(field)\"", with: "\"renamed\"")
        #expect(throws: DecodingError.self) { try JSONDecoder().decode(DeviceList.self, from: Data(text.utf8)) }
    }
}
