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

/// Whether this phone's key is one of the person's devices, and how to get it added.
///
/// A key signs decisions only once it is added to the org. The phone asks to be added; the node keeps
/// the request and shows a pairing code; the person compares that code on a device that is already
/// added (or at the node's terminal) and adds this one. Until then the desk can be read, not answered.
@MainActor
@Observable
public final class PairingStore {
    public enum Status: Equatable, Sendable {
        case checking
        /// The key is a device in these orgs.
        case ready(orgs: [String])
        case notRequested
        /// Asked, and not added yet: the code to compare.
        case waiting(code: String)
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
                status = .waiting(code: request.pairingCode)
            } else {
                status = .notRequested
            }
        } catch {
            status = .failed(DeskStore.reason(error))
        }
    }

    /// Asks the node to add this phone.
    public func requestAdd() async {
        guard !isAsking else { return }
        isAsking = true
        defer { isAsking = false }
        do {
            let request = try await client.requestDevice(label: label, publicKey: key.string)
            status = .waiting(code: request.pairingCode)
        } catch {
            status = .failed(DeskStore.reason(error))
        }
    }
}
