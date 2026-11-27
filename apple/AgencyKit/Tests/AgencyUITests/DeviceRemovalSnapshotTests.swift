#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The devices list with its removal buttons and what is said of a removal, and the confirmation, drawn at phone width. With
/// `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Device removal renderings", .serialized)
struct DeviceRemovalSnapshotTests {
    private func device(_ id: String, _ key: String, _ label: String) -> String {
        #"{"addedAt":"2026-11-24T12:00:00Z","id":"\#(id)","label":"\#(label)","publicKey":"\#(key)"}"#
    }

    private func list(_ orgs: [(String, [String])]) -> DeviceList {
        let body = orgs.map { #"{"org":"\#($0.0)","devices":[\#($0.1.joined(separator: ","))]}"# }.joined(separator: ",")
        return try! JSONDecoder().decode(DeviceList.self, from: Data(#"{"orgs":[\#(body)],"requests":[]}"#.utf8))
    }

    private var devices: DeviceList {
        let own = device("d-own000000001", "own-key", "iPhone of Erin")
        let old = device("d-old000000002", "old-key", UIFixtures.longJapaneseTitle)
        let long = device("d-long00000003", "long-key", UIFixtures.unbrokenID)
        return list([("acme", [own, old, long]), ("beta", [own, old])])
    }

    private func support(
        removing: Set<String> = [], notes: [String: String] = [:], failures: [String: String] = [:], list: DeviceList? = nil
    ) -> RemovalSupport {
        let list = list ?? devices
        return RemovalSupport(
            plan: { DeviceRemovalPlan(for: $0, in: list, ownKey: "own-key") }, removing: removing, notes: notes, failures: failures, onRemove: { _ in }
        )
    }

    private func expectFits(_ result: Snapshot.Result, _ name: String) {
        #expect(!result.png.isEmpty, "\(name) was not drawn")
        #expect(result.size.width == Snapshot.phoneWidth, "\(name) is \(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
    }

    @Test("the list with a button for each device, a removal under way, one the root's node holds, and a failure, with long names")
    func list() {
        let rows = DeviceFacts.rows(devices, ownKey: "own-key")
        let old = rows.first { $0.id == "d-old000000002" }!.id
        let long = rows.first { $0.id == "d-long00000003" }!.id
        let states: [(String, RemovalSupport)] = [
            ("device-removal-buttons", support()),
            ("device-removal-states", support(removing: [old], notes: [long: Ja.Devices.Remove.pendingRoot(command: RootChangeNotice.command)])),
            ("device-removal-failed", support(failures: [old: Ja.Devices.Remove.failed("no such device")])),
        ]
        for (name, removal) in states {
            for scheme in [ColorScheme.light, .dark] {
                expectFits(Snapshot.capture(name, YourDevicesCard(rows: rows, now: Date(), removal: removal).padding(Theme.gutter), scheme: scheme), name)
            }
        }
    }

    @Test("the confirmation for one device in two orgs, for this device, for a device in an org this one is not added to, and for the last device")
    func confirmations() {
        let rows = DeviceFacts.rows(devices, ownKey: "own-key")
        let notAdded = list([("acme", [device("d-own000000001", "own-key", "iPhone of Erin"), device("d-old000000002", "old-key", "Old phone")]), ("beta", [device("d-old000000002", "old-key", "Old phone"), device("d-x", "x-key", "Laptop")])])
        let alone = list([("acme", [device("d-own000000001", "own-key", "iPhone of Erin"), device("d-old000000002", "old-key", "Old phone")]), ("beta", [device("d-old000000002", "old-key", "Old phone")])])
        let cases: [(String, DeviceRemovalPlan)] = [
            ("removal-confirm-two-orgs", DeviceRemovalPlan(for: rows.first { $0.id == "d-old000000002" }!, in: devices, ownKey: "own-key")),
            ("removal-confirm-this-device", DeviceRemovalPlan(for: rows.first { $0.isThisPhone }!, in: devices, ownKey: "own-key")),
            ("removal-confirm-still-in", DeviceRemovalPlan(for: DeviceFacts.rows(notAdded, ownKey: "own-key").first { $0.id == "d-old000000002" }!, in: notAdded, ownKey: "own-key")),
            ("removal-confirm-last", DeviceRemovalPlan(for: DeviceFacts.rows(alone, ownKey: "own-key").first { $0.id == "d-old000000002" }!, in: alone, ownKey: "own-key")),
        ]
        for (name, plan) in cases {
            for scheme in [ColorScheme.light, .dark] {
                expectFits(Snapshot.capture(name, RemovalConfirmation(plan: plan, onConfirm: {}, onCancel: {}), scheme: scheme), name)
            }
        }
    }
}
#endif
