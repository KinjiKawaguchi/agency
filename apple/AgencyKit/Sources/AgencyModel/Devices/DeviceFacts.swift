import AgencyAPI
import Foundation

/// Reading the node's list of devices, as the web client does (`device/enrollment.ts`).
public enum DeviceFacts {
    /// The device id this key has in `org`, when it was added there: only then does the org accept its signatures.
    public static func deviceID(in list: DeviceList, key: String, org: String) -> String? {
        list.orgs.first { $0.org == org }?.devices.first { $0.publicKey == key }?.id
    }

    /// The orgs that do not yet hold this key: a new device is added to each of them.
    public static func orgsLacking(_ list: DeviceList, key: String) -> [String] {
        list.orgs.filter { org in !org.devices.contains { $0.publicKey == key } }.map(\.org)
    }

    /// Requests to add some other device, which this one can confirm.
    public static func requestsFromOthers(_ list: DeviceList, ownKey: String) -> [DeviceRequest] {
        list.requests.filter { $0.publicKey != ownKey }
    }

    /// One row for each device of the person, whatever the number of orgs it is in, the newest first.
    public struct Row: Equatable, Identifiable, Sendable {
        public let id: String
        public let label: String
        public let publicKey: String
        public let addedAt: Instant
        public let orgs: [String]
        public let isThisPhone: Bool
    }

    public static func rows(_ list: DeviceList, ownKey: String) -> [Row] {
        var order: [String] = []
        var byID: [String: (device: Device, orgs: [String])] = [:]
        for org in list.orgs {
            for device in org.devices {
                if byID[device.id] == nil { order.append(device.id) }
                byID[device.id, default: (device, [])].orgs.append(org.org)
            }
        }
        return order.compactMap { id in byID[id] }
            .map { Row(id: $0.device.id, label: $0.device.label, publicKey: $0.device.publicKey, addedAt: $0.device.addedAt, orgs: $0.orgs, isThisPhone: $0.device.publicKey == ownKey) }
            .sorted { $0.addedAt > $1.addedAt }
    }
}
