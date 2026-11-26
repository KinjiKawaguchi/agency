import AgencyAPI
import AgencyProtocol
import Foundation
import Observation

/// What the pairing needs from the node. `NodeClient` is the real one.
public protocol PairingClient: Sendable {
    func devices() async throws -> DeviceList
    func requestDevice(label: String, publicKey: String) async throws -> DeviceRequest
}

extension NodeClient: PairingClient {}

/// How a device that asked to be added gets approved: from one of the person's devices when they have
/// one, and otherwise at the node's machine, because only the account key can add a first device.
public enum ApprovalWay: Equatable, Sendable {
    /// Open Agency on one of these devices (their labels) and approve it under Devices.
    case fromDevices(labels: [String])
    /// The account has no device yet: run this command on the machine the node runs on.
    case atTheNode(command: String)
}

/// Whether this phone's key is one of the person's devices, and how to get it added.
///
/// A key signs decisions only once it is added to the org. The phone asks to be added; the node keeps
/// the request and shows a pairing code; the person compares that code on a device that is already
/// added (or at the node's terminal) and adds this one. Until then the desk can be read, not answered.
@MainActor
@Observable
public final class PairingStore {
    /// What the node's machine is told when it has no device yet and the node says nothing more exact.
    public static let defaultCommand = "agency device-add"

    public enum Status: Equatable, Sendable {
        case checking
        /// The key is a device in these orgs.
        case ready(orgs: [String])
        case notRequested
        /// Asked, and not added yet: the code to compare, and how to approve it.
        case waiting(code: String, approval: ApprovalWay)
        case failed(String)
    }

    public private(set) var status: Status = .checking
    public private(set) var isAsking = false

    @ObservationIgnored private let client: any PairingClient
    @ObservationIgnored private let key: DevicePublicKey
    @ObservationIgnored private let label: String

    /// - Parameters:
    ///   - key: this phone's public key.
    ///   - label: what the other devices show for this one, such as `iPhone` (never the name the owner gave the phone).
    public init(client: any PairingClient, key: DevicePublicKey, label: String) {
        self.client = client
        self.key = key
        self.label = label
    }

    /// This phone's code, which the node computes the same way from the key.
    public var pairingCode: String { key.fingerprint.pairingCode }

    public var isReady: Bool {
        if case .ready = status { true } else { false }
    }

    public func refresh() async {
        do {
            let list = try await client.devices()
            let mine = key.string
            let orgs = list.orgs.filter { $0.devices.contains { $0.publicKey == mine } }.map(\.org)
            if !orgs.isEmpty {
                status = .ready(orgs: orgs)
            } else if let request = list.requests.first(where: { $0.publicKey == mine }) {
                status = .waiting(code: request.pairingCode, approval: Self.approval(in: list))
            } else {
                status = .notRequested
            }
        } catch {
            status = .failed(DeskStore.reason(error))
        }
    }

    /// The person's devices, whatever the number of orgs each is in, oldest first: the places to approve from.
    static func approval(in list: DeviceList) -> ApprovalWay {
        var seen = Set<String>()
        let devices = list.orgs.flatMap(\.devices).sorted { $0.addedAt < $1.addedAt }.filter { seen.insert($0.id).inserted }
        return devices.isEmpty ? .atTheNode(command: command(in: list)) : .fromDevices(labels: devices.map(\.label))
    }

    /// The command the node gave for its machine, or the default. A value that is empty, over 500
    /// characters or holds a control character is not shown: it is only ever displayed, never run, and
    /// it must not be able to hide anything on a screen.
    static func command(in list: DeviceList) -> String {
        guard let command = list.node?.command?.trimmingCharacters(in: .whitespacesAndNewlines), !command.isEmpty,
              command.unicodeScalars.count <= 500,
              !command.unicodeScalars.contains(where: { $0.properties.generalCategory == .control })
        else { return defaultCommand }
        return command
    }

    /// Asks the node to add this phone.
    public func requestAdd() async {
        guard !isAsking else { return }
        isAsking = true
        defer { isAsking = false }
        do {
            let request = try await client.requestDevice(label: label, publicKey: key.string)
            // The list says how it gets approved; without it, the node's machine is the way that always works.
            let list = try? await client.devices()
            status = .waiting(code: request.pairingCode, approval: list.map(Self.approval(in:)) ?? .atTheNode(command: Self.defaultCommand))
        } catch {
            status = .failed(DeskStore.reason(error))
        }
    }
}
