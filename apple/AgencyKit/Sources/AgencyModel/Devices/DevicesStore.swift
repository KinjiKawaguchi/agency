import AgencyAPI
import AgencyProtocol
import Foundation
import Observation

/// Where the devices screen reads from. `NodeClient` is the real one.
public protocol DevicesSource: Sendable {
    func devices() async throws -> DeviceList
}

extension NodeClient: DevicesSource {}

/// Adds another device when the person has compared the codes.
public protocol DeviceAdder: Sendable {
    func add(_ request: DeviceRequest) async throws -> [DeviceChangeResult]
}

extension DeviceChangeSender: DeviceAdder {}

/// The devices screen: the person's devices, the devices waiting to be added, and confirming one.
@MainActor
@Observable
public final class DevicesStore {
    /// How often the open screen asks the node again for its devices and their requests.
    public static let pollSeconds = 4

    public private(set) var rows: [DeviceFacts.Row] = []
    /// Requests to add some other device, which this one can confirm.
    public private(set) var requests: [DeviceRequest] = []
    public private(set) var isLoaded = false
    public private(set) var loadFailure: String?
    /// Requests being confirmed, by key.
    public private(set) var confirming: Set<String> = []
    /// What was said for a confirmed request, by key, until the request leaves the list.
    public private(set) var confirmed: [String: String] = [:]
    /// What was said for a request this device approved that is neither added nor refused (the root's node holds it for its owner),
    /// by key, until the request leaves the list. A request in it cannot be approved again.
    public private(set) var held: [String: String] = [:]
    public private(set) var confirmFailures: [String: String] = [:]

    @ObservationIgnored private let source: any DevicesSource
    @ObservationIgnored private let adder: any DeviceAdder
    @ObservationIgnored private let ownKey: String

    /// A store with nothing to read and nothing to add: for a phone that has no key.
    public static var empty: DevicesStore {
        DevicesStore(source: NoDevices(), adder: NoDevices(), ownKey: nil)
    }

    private init(source: any DevicesSource, adder: any DeviceAdder, ownKey: DevicePublicKey?) {
        self.source = source
        self.adder = adder
        self.ownKey = ownKey?.string ?? ""
    }

    public init(source: any DevicesSource, adder: any DeviceAdder, ownKey: DevicePublicKey) {
        self.source = source
        self.adder = adder
        self.ownKey = ownKey.string
    }

    public func refresh() async {
        do {
            let list = try await source.devices()
            rows = DeviceFacts.rows(list, ownKey: ownKey)
            requests = DeviceFacts.requestsFromOthers(list, ownKey: ownKey)
            let waiting = Set(requests.map(\.publicKey))
            confirmed = confirmed.filter { waiting.contains($0.key) }
            held = held.filter { waiting.contains($0.key) }
            loadFailure = nil
            isLoaded = true
        } catch {
            loadFailure = DeskStore.reason(error)
        }
    }

    /// Adds the device that asked, after the person compared the codes. One request at a time.
    public func confirm(_ request: DeviceRequest) async {
        let key = request.publicKey
        guard !confirming.contains(key) else { return }
        confirming.insert(key)
        confirmFailures[key] = nil
        defer { confirming.remove(key) }
        do {
            say(try await adder.add(request), for: request)
            await refresh()
        } catch let partly as DeviceChangePartlyAnswered {
            say(partly.answered, for: request)
            confirmFailures[key] = DeskStore.reason(partly.cause)
        } catch {
            confirmFailures[key] = DeskStore.reason(error)
        }
    }

    /// What the node answered, said for the request. A change the root's node holds, or an outcome this app does not know but the
    /// node explains, is not a success: it is said as it is and the request is not approved again. Anything else that came back
    /// is an add (`confirmed`), and a node that was asleep says so.
    private func say(_ results: [DeviceChangeResult], for request: DeviceRequest) {
        let heldWords = results.compactMap { result -> String? in
            switch result.outcome {
            case .pending: RootChangeNotice.approvedHere
            case .unknown: result.message
            case .applied, .delivered, .queued: nil
            }
        }
        let queuedWords = results.first { $0.outcome == .queued }.map { Ja.Devices.queued($0.message) }
        guard !heldWords.isEmpty else {
            confirmed[request.publicKey] = queuedWords ?? Ja.Devices.confirmed(request.label)
            return
        }
        var lines: [String] = []
        for line in heldWords + [queuedWords].compactMap({ $0 }) where !lines.contains(line) { lines.append(line) }
        held[request.publicKey] = lines.joined(separator: "\n")
    }
}

private struct NoDevices: DevicesSource, DeviceAdder {
    func devices() async throws -> DeviceList { throw NodeError.notSignedIn }
    func add(_ request: DeviceRequest) async throws -> [DeviceChangeResult] { throw NodeError.notSignedIn }
}
