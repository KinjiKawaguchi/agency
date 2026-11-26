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

/// The person's devices, whatever the number of orgs each is in.
struct YourDevicesCard: View {
    let rows: [DeviceFacts.Row]
    let now: Date

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
                }
                if row.id != rows.last?.id { Divider() }
            }
        }
        .card()
    }
}

/// A device that asked to be added: its code, to compare with the one on the new device, and the button.
struct RequestCard: View {
    let request: DeviceRequest
    let isConfirming: Bool
    let confirmed: String?
    let failure: String?
    let now: Date
    let onConfirm: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            HStack(spacing: 8) {
                Text(request.label).font(.subheadline.weight(.semibold)).lineLimit(2).wrapping()
                Text(Ja.Devices.requestedAt(Ago.text(from: request.requestedAt.date, now: now))).font(.footnote).foregroundStyle(.secondary)
            }
            CodeView(code: request.pairingCode, caption: Ja.Devices.codeOf(request.label))
            Text(Ja.Devices.compare).font(.footnote).foregroundStyle(Theme.warning).wrapping()
            if let confirmed {
                Label(confirmed, systemImage: "checkmark.circle.fill").font(.footnote).foregroundStyle(.green).wrapping()
            } else {
                Button(action: onConfirm) {
                    Text(isConfirming ? Ja.Devices.confirming : Ja.Devices.confirm)
                        .font(.headline).multilineTextAlignment(.center).lineLimit(3).wrapping().frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(isConfirming)
                Label(Ja.Devices.faceID, systemImage: "faceid").font(.caption).foregroundStyle(.secondary).wrapping()
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
    let confirmFailures: [String: String]
    let now: Date
    let onAdd: () -> Void
    let onCheck: () -> Void
    let onConfirm: (DeviceRequest) -> Void

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
                if !rows.isEmpty { YourDevicesCard(rows: rows, now: now) }
                Text(Ja.Devices.requests).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).padding(.top, 8).wrapping()
                if requests.isEmpty {
                    Text(Ja.Devices.noRequests).font(.footnote).foregroundStyle(.secondary).wrapping()
                }
                ForEach(requests, id: \.publicKey) { request in
                    RequestCard(
                        request: request, isConfirming: confirming.contains(request.publicKey),
                        confirmed: confirmed[request.publicKey], failure: confirmFailures[request.publicKey], now: now
                    ) { onConfirm(request) }
                }
            }
        }
    }
}

/// The devices tab: this phone and the others, and confirming a device that asked to be added.
public struct DevicesScreen: View {
    private let store: DevicesStore
    private let pairing: PairingStore?
    private let notice: String?

    /// - Parameters:
    ///   - pairing: this phone's state; `nil` when it has no key.
    ///   - notice: something the app has to say, such as why there is no key.
    public init(store: DevicesStore?, pairing: PairingStore?, notice: String?) {
        self.store = store ?? DevicesStore.empty
        self.pairing = pairing
        self.notice = notice
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.spacing) {
                    if let notice { Text(notice).font(.subheadline).foregroundStyle(Theme.danger).wrapping().card() }
                    if let pairing {
                        DevicesContent(
                            code: pairing.pairingCode, pairing: pairing.status, isAsking: pairing.isAsking,
                            rows: store.rows, requests: store.requests, isLoaded: store.isLoaded, loadFailure: store.loadFailure,
                            confirming: store.confirming, confirmed: store.confirmed, confirmFailures: store.confirmFailures, now: Date(),
                            onAdd: { Task { await pairing.requestAdd() } },
                            onCheck: { Task { await refresh(pairing) } },
                            onConfirm: { request in Task { await store.confirm(request) } }
                        )
                    }
                }
                .padding(Theme.gutter)
            }
            .refreshable { if let pairing { await refresh(pairing) } }
            .navigationTitle(Ja.Devices.title)
        }
        .task {
            // The screen asks again while it is open, so a device that asks to be added, or is approved,
            // shows up without a reload.
            while !Task.isCancelled {
                if let pairing { await refresh(pairing) }
                try? await Task.sleep(for: .seconds(DevicesStore.pollSeconds))
            }
        }
    }

    private func refresh(_ pairing: PairingStore) async {
        await pairing.refresh()
        await store.refresh()
    }
}
#endif
