import AgencyAPI
import AgencyProtocol
import Foundation
import Observation

/// Where the devices screen reads from. `NodeClient` is the real one.
public protocol DevicesSource: Sendable {
    func devices() async throws -> DeviceList
    /// The changes the root's node holds for its owner to sign (`Me.rootChanges`), so that a request this device approved is still
    /// known to be held after the screen is opened again.
    func heldChanges() async throws -> [RootChangeRef]
}

extension NodeClient: DevicesSource {
    public func heldChanges() async throws -> [RootChangeRef] { try await me().rootChanges }
}

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
    /// This phone's id in the node's list of devices, once the list has been read and holds it: what a push registration names.
    public var thisDeviceID: String? { rows.first { $0.isThisPhone }?.id }
    /// Requests to add some other device, which this one can confirm.
    public private(set) var requests: [DeviceRequest] = []
    public private(set) var isLoaded = false
    public private(set) var loadFailure: String?
    /// Requests being confirmed, by key.
    public private(set) var confirming: Set<String> = []
    /// What was said for a confirmed request, by key, until the request leaves the list.
    public private(set) var confirmed: [String: String] = [:]
    /// What was said, by key, for a request this device approved just now that is neither added nor refused (the root's node holds it
    /// for its owner), until the request leaves the list.
    public private(set) var heldHere: [String: String] = [:]
    /// The ids of the devices the node says it holds an add of for the root key's owner (`Me.rootChanges`), as of the last read.
    private var heldAtNode: Set<String> = []
    public private(set) var confirmFailures: [String: String] = [:]

    /// The devices being removed, by row id. One removal at a time for each.
    public private(set) var removing: Set<String> = []
    /// What was said of a removal the root's node holds for its owner, or that the node explained, by row id, until the device leaves the list:
    /// it is neither removed nor refused, and is not asked for again.
    public private(set) var removalNotes: [String: String] = [:]
    public private(set) var removalFailures: [String: String] = [:]
    /// What was said of the last removal that took effect. The device has left the list by then, so it has no row to say it on.
    public private(set) var removedNotice: String?
    /// This device's own removal took effect (it is not held by the root's node): its key can never sign again, and the shell makes a new one.
    public private(set) var didRemoveThisDevice = false

    @ObservationIgnored private let source: any DevicesSource
    @ObservationIgnored private let adder: any DeviceAdder
    @ObservationIgnored private let ownKey: String
    @ObservationIgnored private let remover: (any DeviceRemover)?
    @ObservationIgnored private let keptDecisions: @MainActor (_ deviceID: String) -> KeptDecisions
    /// The node's list as last read, which a removal is worked out from.
    @ObservationIgnored private var list: DeviceList?

    /// A store with nothing to read and nothing to add: for a phone that has no key.
    public static var empty: DevicesStore {
        DevicesStore(source: NoDevices(), adder: NoDevices(), ownKey: nil)
    }

    private init(source: any DevicesSource, adder: any DeviceAdder, ownKey: DevicePublicKey?) {
        self.source = source
        self.adder = adder
        self.ownKey = ownKey?.string ?? ""
        remover = nil
        keptDecisions = { _ in .noneKept }
    }

    /// - Parameters:
    ///   - remover: removes a device of the person's; `nil` where this build has no way to (the removal is then not offered).
    ///   - keptDecisions: what is known of the decisions signed by the device with this id that are kept on this phone, waiting to be sent
    ///     (this device, or an earlier key of it that was made again), or that cannot be read now. A device is not removed while any are,
    ///     since removing its key would make each of them fail for good.
    public init(
        source: any DevicesSource, adder: any DeviceAdder, ownKey: DevicePublicKey, remover: (any DeviceRemover)? = nil,
        keptDecisions: @escaping @MainActor (_ deviceID: String) -> KeptDecisions = { _ in .noneKept }
    ) {
        self.keptDecisions = keptDecisions
        self.source = source
        self.adder = adder
        self.ownKey = ownKey.string
        self.remover = remover
    }

    public func refresh() async {
        do {
            let list = try await source.devices()
            self.list = list
            rows = DeviceFacts.rows(list, ownKey: ownKey)
            requests = DeviceFacts.requestsFromOthers(list, ownKey: ownKey)
            let waiting = Set(requests.map(\.publicKey))
            confirmed = confirmed.filter { waiting.contains($0.key) }
            heldHere = heldHere.filter { waiting.contains($0.key) }
            // A removal's note goes with the device: it is gone when the node stops listing it, and not before.
            let listed = Set(rows.map(\.id))
            removalNotes = removalNotes.filter { listed.contains($0.key) }
            removalFailures = removalFailures.filter { listed.contains($0.key) }
            await readHeldAtNode()
            loadFailure = nil
            isLoaded = true
        } catch {
            loadFailure = DeskStore.reason(error)
        }
    }

    public func dismissRemovedNotice() { removedNotice = nil }

    /// Whether a device can be removed from here: this build has a way to, and the node's list has been read.
    public var offersRemoval: Bool { remover != nil && list != nil }

    /// What removing this device comes to, from the node's list as last read.
    public func removalPlan(for row: DeviceFacts.Row) -> DeviceRemovalPlan? {
        list.map { DeviceRemovalPlan(for: row, in: $0, ownKey: ownKey, keptDecisions: keptDecisions(row.id)) }
    }

    /// Removes the device, after the person has confirmed. The device leaves the list when the node stops listing it: not before, and not
    /// while the root's node holds the removal.
    public func remove(_ row: DeviceFacts.Row) async {
        guard let remover, let plan = removalPlan(for: row), !removing.contains(row.id), removalNotes[row.id] == nil else { return }
        removalFailures[row.id] = nil
        guard !plan.isBlocked else {
            removalFailures[row.id] = Ja.Devices.Remove.failed(plan.refusal ?? Ja.Devices.Remove.cannotHere)
            return
        }
        removing.insert(row.id)
        defer { removing.remove(row.id) }
        // The list the person confirmed may be old, and the rule about the last device is the client's: it is read again, and the plan
        // is made again from it, before anything is signed. A plan that is not what was confirmed is not carried out.
        await refresh()
        guard loadFailure == nil, let current = removalPlan(for: row), current == plan else {
            removalFailures[row.id] = loadFailure.map(Ja.Devices.Remove.failed) ?? Ja.Devices.Remove.listChanged
            return
        }
        do {
            let results = try await remover.remove(plan)
            say(results, of: plan, row: row, isComplete: true)
            await refresh()
        } catch let partly as DeviceChangePartlyAnswered {
            say(partly.answered, of: plan, row: row, isComplete: false)
            removalFailures[row.id] = Ja.Devices.Remove.failed(DeskStore.reason(partly.cause))
        } catch {
            removalFailures[row.id] = Ja.Devices.Remove.failed(DeskStore.reason(error))
        }
    }

    /// What the node answered, said for the removal. A removal the root's node holds, or an outcome this app does not know that the node
    /// explains, is said as it is and kept on the device's row; anything else that came back took effect, and a node that was asleep says so.
    private func say(_ results: [DeviceChangeResult], of plan: DeviceRemovalPlan, row: DeviceFacts.Row, isComplete: Bool) {
        let held = results.compactMap { result -> String? in
            switch result.outcome {
            case .pending: Ja.Devices.Remove.pendingRoot(command: RootChangeNotice.command)
            case .unknown: Ja.Devices.unknownOutcome(result.message)
            case .applied, .delivered, .queued: nil
            }
        }
        let queued = results.first { $0.outcome == .queued }.map { Ja.Devices.queued($0.message) }
        var lines: [String] = []
        for line in held + [queued].compactMap({ $0 }) where !lines.contains(line) { lines.append(line) }
        if !held.isEmpty {
            removalNotes[row.id] = lines.joined(separator: "\n")
            return
        }
        // A new key is made only when every org took the removal: with one that failed, the key still signs there and can be tried again.
        if plan.isThisDevice && isComplete { didRemoveThisDevice = true }
        var notice = [Ja.Devices.Remove.removed(plan.label)] + lines
        if !plan.stillIn.isEmpty { notice.append(Ja.Devices.Remove.stillIn(plan.stillIn.joined(separator: "、"))) }
        removedNotice = notice.joined(separator: "\n")
    }

    /// What is said for a request that is held, by key: what this device was told when it approved it, and for a request approved
    /// earlier (before the app was closed, or from another device) what the node's list of held changes says of it. A request in it
    /// cannot be approved again.
    public var held: [String: String] {
        var words = heldHere
        for request in requests where heldAtNode.contains(request.deviceId) && words[request.publicKey] == nil {
            words[request.publicKey] = RootChangeNotice.approvedHere
        }
        return words
    }

    /// A read that fails leaves what was known: the screen is about devices, and a held change it cannot read is not an error here.
    private func readHeldAtNode() async {
        guard let changes = try? await source.heldChanges() else { return }
        heldAtNode = Set(changes.filter { $0.kind == "addDevice" }.compactMap(\.device))
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
            case .unknown: Ja.Devices.unknownOutcome(result.message)
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
        heldHere[request.publicKey] = lines.joined(separator: "\n")
    }
}

private struct NoDevices: DevicesSource, DeviceAdder {
    func devices() async throws -> DeviceList { throw NodeError.notSignedIn }
    func heldChanges() async throws -> [RootChangeRef] { throw NodeError.notSignedIn }
    func add(_ request: DeviceRequest) async throws -> [DeviceChangeResult] { throw NodeError.notSignedIn }
}
