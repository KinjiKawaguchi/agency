#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The code to compare, large enough to read across a table.
struct CodeView: View {
    let code: String
    let caption: String

    var body: some View {
        VStack(spacing: 4) {
            Text(caption).font(.caption).foregroundStyle(.secondary)
            Text(code)
                .font(.system(size: 30, weight: .bold, design: .monospaced))
                .minimumScaleFactor(0.6)
                .lineLimit(1)
                .accessibilityLabel(code.replacingOccurrences(of: "-", with: " "))
        }
        .frame(maxWidth: .infinity)
    }
}

/// This phone: its code, and whether it is added.
struct ThisDeviceCard: View {
    let code: String
    let pairing: PairingStore.Status
    let isAsking: Bool
    let deviceID: String?
    let onAdd: () -> Void
    let onCheck: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Devices.thisDevice).font(.headline)
            CodeView(code: code, caption: Ja.Devices.codeLabel)
            if case .ready = pairing {
                Label(Ja.Devices.added, systemImage: "checkmark.circle.fill").font(.subheadline).foregroundStyle(.green).wrapping()
                if let deviceID {
                    Text(Ja.Devices.addedAs(deviceID)).font(.footnote).foregroundStyle(.secondary).lineLimit(2).truncationMode(.middle).wrapping()
                }
            } else {
                PairingCard(status: pairing, isAsking: isAsking, onAdd: onAdd, onCheck: onCheck)
            }
        }
        .card()
    }
}

/// What the devices list needs to offer the removal of a device: what it comes to for a row, what is under way, and what was said.
struct RemovalSupport {
    let plan: (DeviceFacts.Row) -> DeviceRemovalPlan?
    let removing: Set<String>
    /// What was said of a removal that is held or that the node explained, by row id.
    let notes: [String: String]
    let failures: [String: String]
    let onRemove: (DeviceFacts.Row) -> Void
}

/// The person's devices, whatever the number of orgs each is in.
struct YourDevicesCard: View {
    let rows: [DeviceFacts.Row]
    let now: Date
    /// `nil` where the removal is not offered.
    var removal: RemovalSupport?
    @State private var asking: DeviceFacts.Row?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(Ja.Devices.yourDevices).font(.headline)
            ForEach(rows) { row in
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 8) {
                        Text(row.label).font(.subheadline.weight(.semibold)).lineLimit(2).wrapping()
                        if row.isThisPhone { Tag(text: Ja.Devices.thisOne) }
                    }
                    Text("\(Ja.Devices.addedOn(Ago.text(from: row.addedAt.date, now: now))) · \(Ja.Devices.inOrgs(row.orgs.joined(separator: "、")))")
                        .font(.footnote).foregroundStyle(.secondary).wrapping()
                    Text(row.id).font(.caption.monospaced()).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                    if let removal { removalLines(for: row, removal) }
                }
                if row.id != rows.last?.id { Divider() }
            }
        }
        .card()
        .sheet(item: $asking) { row in
            if let plan = removal?.plan(row) {
                RemovalConfirmation(plan: plan) {
                    asking = nil
                    removal?.onRemove(row)
                } onCancel: {
                    asking = nil
                }
            }
        }
    }

    /// The removal of one device: its button, or that it is being removed, or what the node said of it, and why it failed.
    @ViewBuilder
    private func removalLines(for row: DeviceFacts.Row, _ removal: RemovalSupport) -> some View {
        if removal.removing.contains(row.id) {
            Text(Ja.Devices.Remove.removing).font(.footnote).foregroundStyle(.secondary)
        } else if let note = removal.notes[row.id] {
            Label(note, systemImage: "hourglass").font(.footnote).foregroundStyle(.secondary).wrapping()
        } else {
            Button(role: .destructive) { asking = row } label: { Text(Ja.Devices.Remove.button).font(.footnote.weight(.semibold)) }
                .accessibilityLabel(Ja.Devices.Remove.buttonFor(row.label))
                .padding(.top, 2)
        }
        if let failure = removal.failures[row.id] {
            Label(failure, systemImage: "exclamationmark.triangle.fill").font(.footnote).foregroundStyle(Theme.danger).wrapping()
        }
    }
}

/// A device that asked to be added: its code, to compare with the one on the new device, and the button.
struct RequestCard: View {
    let request: DeviceRequest
    let isConfirming: Bool
    let confirmed: String?
    /// What was said for a change the root's node holds for its owner.
    let held: String?
    let failure: String?
    let now: Date
    let onConfirm: () -> Void
    @Environment(\.keyProtection) private var protection

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            HStack(spacing: 8) {
                Text(request.label).font(.subheadline.weight(.semibold)).lineLimit(2).wrapping()
                Text(Ja.Devices.requestedAt(Ago.text(from: request.requestedAt.date, now: now))).font(.footnote).foregroundStyle(.secondary)
            }
            CodeView(code: request.pairingCode, caption: Ja.Devices.codeOf(request.label))
            Text(Ja.Devices.compare).font(.footnote).foregroundStyle(Theme.warning).wrapping()
            if let held {
                // Not a success and not a refusal: the change waits for the root key's owner.
                Label(held, systemImage: "hourglass").font(.footnote).foregroundStyle(.secondary).wrapping()
            }
            if let confirmed {
                Label(confirmed, systemImage: "checkmark.circle.fill").font(.footnote).foregroundStyle(.green).wrapping()
            } else if held == nil || failure != nil {
                Button(action: onConfirm) {
                    Text(isConfirming ? Ja.Devices.confirming : Ja.Devices.confirm)
                        .font(.headline).multilineTextAlignment(.center).lineLimit(3).wrapping().frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(isConfirming)
                Label(Ja.Key.confirmAdd(protection), systemImage: Ja.Key.symbol(protection)).font(.caption).foregroundStyle(.secondary).wrapping()
            }
            if let failure {
                Label(Ja.Devices.confirmFailed(failure), systemImage: "exclamationmark.triangle.fill")
                    .font(.footnote).foregroundStyle(Theme.danger).wrapping()
            }
        }
        .card()
    }
}

/// The whole screen's content, without the navigation around it.
struct DevicesContent: View {
    let code: String
    let pairing: PairingStore.Status
    let isAsking: Bool
    let rows: [DeviceFacts.Row]
    let requests: [DeviceRequest]
    let isLoaded: Bool
    let loadFailure: String?
    let confirming: Set<String>
    let confirmed: [String: String]
    let held: [String: String]
    let confirmFailures: [String: String]
    let now: Date
    let onAdd: () -> Void
    let onCheck: () -> Void
    let onConfirm: (DeviceRequest) -> Void
    /// What the removal of a device needs; `nil` where it is not offered.
    var removal: RemovalSupport?

    private var isAdded: Bool { if case .ready = pairing { true } else { false } }
    private var thisDeviceID: String? { rows.first(where: \.isThisPhone)?.id }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Devices.intro).font(.footnote).foregroundStyle(.secondary).wrapping()
            if let loadFailure {
                Text(Ja.Devices.listFailed(loadFailure)).font(.footnote).foregroundStyle(Theme.danger).wrapping()
            }
            ThisDeviceCard(code: code, pairing: pairing, isAsking: isAsking, deviceID: thisDeviceID, onAdd: onAdd, onCheck: onCheck)
            if !isLoaded && loadFailure == nil {
                Text(Ja.Devices.loadingList).font(.footnote).foregroundStyle(.secondary).wrapping()
            }
            if isAdded {
                if !rows.isEmpty { YourDevicesCard(rows: rows, now: now, removal: removal) }
                Text(Ja.Devices.requests).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).padding(.top, 8).wrapping()
                if isLoaded && requests.isEmpty {
                    Text(Ja.Devices.noRequests).font(.footnote).foregroundStyle(.secondary).wrapping()
                }
                ForEach(requests, id: \.publicKey) { request in
                    RequestCard(
                        request: request, isConfirming: confirming.contains(request.publicKey),
                        confirmed: confirmed[request.publicKey], held: held[request.publicKey], failure: confirmFailures[request.publicKey], now: now
                    ) { onConfirm(request) }
                }
            }
        }
    }
}

/// What the devices screen needs to say that this device's key cannot be used, and to offer the two ways on.
public struct KeyUnusableSupport {
    public let problem: KeyProblem
    public let onTryAgain: () -> Void
    public let onMakeNew: () -> Void

    public init(problem: KeyProblem, onTryAgain: @escaping () -> Void, onMakeNew: @escaping () -> Void) {
        self.problem = problem
        self.onTryAgain = onTryAgain
        self.onMakeNew = onMakeNew
    }
}

/// The devices tab: this phone and the others, and confirming a device that asked to be added.
public struct DevicesScreen: View {
    private let store: DevicesStore
    private let pairing: PairingStore?
    private let notice: String?
    private let noticeIsFailure: Bool
    private let pushDelivery: PushDeliveryStore?
    private let keyUnusable: KeyUnusableSupport?

    /// - Parameters:
    ///   - keyUnusable: set when this device's key is stored and cannot be used now: the screen says so, and offers to try again or to make a new key.
    ///   - pairing: this phone's state; `nil` when it has no key.
    ///   - notice: something the app has to say, such as why there is no key.
    ///   - pushDelivery: whether APNs takes the notices the node sends this phone; `nil` for a build that has none to ask.
    ///   - noticeIsFailure: whether the notice is a trouble (red), and not only something the person should know (a key made again).
    public init(
        store: DevicesStore?, pairing: PairingStore?, notice: String?, noticeIsFailure: Bool = true, pushDelivery: PushDeliveryStore? = nil,
        keyUnusable: KeyUnusableSupport? = nil
    ) {
        self.keyUnusable = keyUnusable
        self.store = store ?? DevicesStore.empty
        self.pairing = pairing
        self.notice = notice
        self.noticeIsFailure = noticeIsFailure
        self.pushDelivery = pushDelivery
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.spacing) {
                    if let keyUnusable { KeyUnusableCard(problem: keyUnusable.problem, onTryAgain: keyUnusable.onTryAgain, onMakeNew: keyUnusable.onMakeNew) }
                    if let notice { Text(notice).font(.subheadline).foregroundStyle(noticeIsFailure ? Theme.danger : Color.primary).wrapping().card() }
                    if let pushDelivery, let lines = pushDelivery.lines { PushDeliveryCard(lines: lines) }
                    if let removed = store.removedNotice {
                        VStack(alignment: .leading, spacing: 6) {
                            Label(removed, systemImage: "checkmark.circle.fill").font(.subheadline).foregroundStyle(.green).wrapping()
                            Button(Ja.Desk.close) { store.dismissRemovedNotice() }.font(.footnote)
                        }
                        .card()
                    }
                    if let pairing {
                        DevicesContent(
                            code: pairing.pairingCode, pairing: pairing.status, isAsking: pairing.isAsking,
                            rows: store.rows, requests: store.requests, isLoaded: store.isLoaded, loadFailure: store.loadFailure,
                            confirming: store.confirming, confirmed: store.confirmed, held: store.held, confirmFailures: store.confirmFailures, now: Date(),
                            onAdd: { Task { await pairing.requestAdd() } },
                            onCheck: { Task { await refresh(pairing) } },
                            onConfirm: { request in Task { await store.confirm(request) } },
                            removal: store.offersRemoval ? RemovalSupport(
                                plan: { store.removalPlan(for: $0) }, removing: store.removing, notes: store.removalNotes,
                                failures: store.removalFailures, onRemove: { row in Task { await store.remove(row) } }
                            ) : nil
                        )
                    }
                }
                .padding(Theme.gutter)
            }
            .refreshable {
                if let pairing { await refresh(pairing) }
                await pushDelivery?.refresh(force: true)
            }
            .navigationTitle(Ja.Devices.title)
        }
        .task {
            // The screen asks again while it is open, so a device that asks to be added, or is approved,
            // shows up without a reload.
            while !Task.isCancelled {
                if let pairing { await refresh(pairing) }
                // The read of the node's list is its own, kept for a while: this loop asks every few seconds.
                await pushDelivery?.refresh()
                try? await Task.sleep(for: .seconds(DevicesStore.pollSeconds))
            }
        }
    }

    private func refresh(_ pairing: PairingStore) async {
        await pairing.refresh()
        await store.refresh()
    }
}

/// What removing a device means, said before it is done, with the way to confirm it (when it can be made) and the way back. The question
/// names the device, the message says that it can never sign again and where it is removed, and a removal the app does not make says why.
struct RemovalConfirmation: View {
    let plan: DeviceRemovalPlan
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(plan.title).font(.title3.weight(.semibold)).wrapping()
            ForEach(Array(plan.lines.enumerated()), id: \.offset) { _, line in
                Text(line).font(.subheadline).wrapping()
            }
            if plan.canConfirm {
                Button(role: .destructive, action: onConfirm) {
                    Text(Ja.Devices.Remove.confirm).font(.headline).frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .tint(Theme.danger)
            }
            Button(action: onCancel) {
                Text(plan.canConfirm ? Ja.Devices.Remove.cancel : Ja.Desk.close).font(.headline).frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
        }
        .padding(Theme.gutter)
        .presentationDetents([.medium, .large])
    }
}
#endif
