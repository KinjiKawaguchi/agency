#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The devices screen at phone width, from fixtures only.
@MainActor
@Suite("Devices renderings", .serialized)
struct DevicesSnapshotTests {
    private let now = UIFixtures.now
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
    }

    private func list(_ name: String = "device-list") -> DeviceList {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let data = try! Data(contentsOf: url.appendingPathComponent("testdata/client/\(name).json"))
        return try! JSONDecoder().decode(DeviceList.self, from: data)
    }

    private func content(
        pairing: PairingStore.Status, code: String = "a652-124a-fda7", rows: [DeviceFacts.Row] = [], requests: [DeviceRequest] = [],
        confirming: Set<String> = [], confirmed: [String: String] = [:], failures: [String: String] = [:], isAsking: Bool = false
    ) -> DevicesContent {
        DevicesContent(
            code: code, pairing: pairing, isAsking: isAsking, rows: rows, requests: requests, isLoaded: true, loadFailure: nil,
            confirming: confirming, confirmed: confirmed, confirmFailures: failures, now: now, onAdd: {}, onCheck: {}, onConfirm: { _ in }
        )
    }

    @Test("this phone before it is added, asking, and waiting with its code")
    func beforeAdded() {
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("devices-not-added", content(pairing: .notRequested), scheme: scheme), "not added (\(scheme))")
        }
        expectFits(Snapshot.capture("devices-waiting", content(pairing: .waiting(code: "a652-124a-fda7", approval: .fromDevices(labels: ["MacBook", "iPad"])))), "waiting")
        expectFits(Snapshot.capture("devices-asking", content(pairing: .notRequested, isAsking: true)), "asking")
    }

    @Test("this phone added: its devices and a device that asks to be added")
    func added() {
        let devices = list()
        let rows = DeviceFacts.rows(devices, ownKey: devices.orgs[0].devices[0].publicKey)
        let requests = DeviceFacts.requestsFromOthers(devices, ownKey: "none")
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("devices-added", content(pairing: .ready(orgs: ["acme"]), code: "dbc2-9825-1c51", rows: rows, requests: requests), scheme: scheme), "added (\(scheme))")
        }
    }

    @Test("a request being confirmed, confirmed, queued, and failed")
    func requestStates() {
        let devices = list()
        let rows = DeviceFacts.rows(devices, ownKey: devices.orgs[0].devices[0].publicKey)
        let requests = DeviceFacts.requestsFromOthers(devices, ownKey: "none")
        let key = requests[0].publicKey
        func shot(_ name: String, confirming: Set<String> = [], confirmed: [String: String] = [:], failures: [String: String] = [:]) {
            expectFits(Snapshot.capture(name, content(pairing: .ready(orgs: ["acme"]), code: "dbc2-9825-1c51", rows: rows, requests: requests, confirming: confirming, confirmed: confirmed, failures: failures)), name)
        }
        shot("devices-confirming", confirming: [key])
        shot("devices-confirmed", confirmed: [key: Ja.Devices.confirmed("iPhone")])
        shot("devices-queued", confirmed: [key: Ja.Devices.queued("node asleep")])
        shot("devices-failed", failures: [key: Ja.Devices.codeMismatch])
    }

    @Test("a very long label and org names wrap and never widen the screen")
    func longNames() throws {
        let long = String(repeating: "とても長い名前の端末", count: 8) + UIFixtures.unbrokenID
        let json = """
        {"orgs":[{"org":"acme","devices":[{"addedAt":"2026-11-24T12:00:00Z","id":"d-a652124afda7","label":"\(long)","publicKey":"k"}]}],
         "requests":[{"deviceId":"d-506ef1879d74","label":"\(long)","pairingCode":"506e-f187-9d74","publicKey":"q","requestedAt":"2026-11-24T12:00:00Z"}]}
        """
        let devices = try JSONDecoder().decode(DeviceList.self, from: Data(json.utf8))
        let view = content(pairing: .ready(orgs: ["acme"]), rows: DeviceFacts.rows(devices, ownKey: "k"), requests: devices.requests)
        expectFits(Snapshot.capture("devices-long", view), "long names")
        expectFits(Snapshot.capture("devices-long", view, scheme: .dark), "long names (dark)")
    }
}
#endif
