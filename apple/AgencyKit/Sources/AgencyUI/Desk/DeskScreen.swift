#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The desk tab: the pile, a request open, and the banners that say what is wrong.
public struct DeskScreen: View {
    private let store: DeskStore
    private let pairing: PairingStore?
    private let notice: String?
    private let onSignInAgain: () -> Void
    private let onSettings: () -> Void
    @Binding private var openItemID: String?
    @State private var path: [String] = []

    /// - Parameters:
    ///   - pairing: whether this phone is added, and how to get it added; `nil` when this phone has no key.
    ///   - notice: something the app has to say above the desk, such as why there is no key.
    ///   - openItemID: a request to open (from a notification); the screen opens it and sets this back to `nil`.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    ///   - onSettings: opens the settings.
    public init(
        store: DeskStore, pairing: PairingStore?, notice: String? = nil, openItemID: Binding<String?>,
        onSignInAgain: @escaping () -> Void, onSettings: @escaping () -> Void
    ) {
        self.store = store
        self.pairing = pairing
        self.notice = notice
        _openItemID = openItemID
        self.onSignInAgain = onSignInAgain
        self.onSettings = onSettings
    }

    public var body: some View {
        NavigationStack(path: $path) {
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.spacing) {
                    banners
                    SendingTray(
                        entries: store.outbox.entries, sending: store.outbox.sending, needsSignIn: store.outbox.needsSignIn,
                        onSendAgain: { id in Task { await store.outbox.sendAgain(id) } },
                        onDiscard: { id in store.discardDecision(id) }
                    )
                    if store.hasAnswer {
                        DeskPile(items: store.items, situation: store.situation, now: Date())
                        if !store.recent.isEmpty || store.recentFailure != nil {
                            RecentDecisions(entries: store.recent, failure: store.recentFailure, now: Date()).padding(.top, 16)
                        }
                    } else if store.loading == .idle || store.loading == .loading {
                        // Until the node answers there is no pile to say is empty.
                        ForEach(0..<3, id: \.self) { _ in SkeletonCard() }
                    }
                }
                .padding(Theme.gutter)
            }
            .refreshable {
                await store.refresh()
                await pairing?.refresh()
            }
            .navigationTitle(Ja.Desk.title)
            .navigationDestination(for: String.self) { id in
                RequestDetailScreen(
                    store: store, isDeviceReady: pairing?.isReady ?? false, isDeviceChecked: pairing.map { $0.status != .checking } ?? true,
                    itemID: id, onMoveOn: moveOn
                )
            }
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: onSettings) { Image(systemName: "gearshape") }
                        .accessibilityLabel(Ja.Desk.settings)
                }
            }
        }
        .onChange(of: openItemID, initial: true) { _, id in
            guard let id else { return }
            path = [id]
            openItemID = nil
        }
        .task {
            await pairing?.refresh()
            await store.refresh()
        }
        // A phone waiting to be approved asks again by itself, so the card goes away when it is.
        .task(id: isWaitingToBeAdded) {
            while isWaitingToBeAdded, !Task.isCancelled {
                try? await Task.sleep(for: .seconds(4))
                await pairing?.refresh()
            }
        }
    }

    /// A decision of the open item is kept: the next item opens at once, or the pile shows when no other waits for the owner.
    private func moveOn(to next: String?) {
        if let next { path = [next] } else { path = [] }
    }

    private var isWaitingToBeAdded: Bool {
        if case .waiting? = pairing?.status { true } else { false }
    }

    @ViewBuilder
    private var banners: some View {
        // A started recovery comes first and cannot be hidden: the owner can cancel it only while it waits.
        ForEach(store.recoveries) { RecoveryBanner(notice: $0) }
        if let rootChanges = store.rootChanges { RootChangeBanner(notice: rootChanges) }
        if let notice {
            Text(notice).font(.subheadline).foregroundStyle(Theme.danger).wrapping().card()
        }
        if let pairing {
            PairingCard(
                status: pairing.status, isAsking: pairing.isAsking,
                onAdd: { Task { await pairing.requestAdd() } },
                onCheck: { Task { await pairing.refresh() } }
            )
        }
        if store.needsSignIn {
            VStack(alignment: .leading, spacing: 8) {
                Text(Ja.Desk.signInEnded).font(.subheadline).wrapping()
                Button(Ja.Desk.signInAgain, action: onSignInAgain).buttonStyle(.borderedProminent)
            }
            .card()
        } else if case .failed(let reason) = store.loading {
            VStack(alignment: .leading, spacing: 8) {
                Text(Ja.Desk.loadFailed(reason)).font(.subheadline).foregroundStyle(Theme.danger).wrapping()
                Button(Ja.Desk.reload) { Task { await store.refresh() } }.buttonStyle(.bordered)
            }
            .card()
        }
        ForEach(store.teamFailures.sorted(by: { $0.key < $1.key }), id: \.key) { team, reason in
            Text(Ja.Desk.unreadableTeam(team, reason: reason)).font(.footnote).foregroundStyle(Theme.warning).wrapping()
        }
        ForEach(store.teams.filter(\.isReplica), id: \.key) { team in
            Text(replicaNotice(team)).font(.footnote).foregroundStyle(.secondary).wrapping()
        }
    }

    private func replicaNotice(_ team: Team) -> String {
        guard case .replica(let fetchedAt, _, let error) = team.source else { return "" }
        let reason = error ?? "—"
        guard let fetchedAt else { return Ja.Desk.neverRead(reason: reason) }
        return Ja.Desk.staleCopy(when: Ago.text(from: fetchedAt.date, now: Date()), reason: reason)
    }
}

/// A request open, wired to the store: the draft lives there, and so does the sending.
struct RequestDetailScreen: View {
    let store: DeskStore
    let isDeviceReady: Bool
    let isDeviceChecked: Bool
    let itemID: String
    /// Called once a decision of this item is kept: the next item to open, or `nil` to go back to the pile.
    let onMoveOn: (String?) -> Void

    var body: some View {
        ScrollView {
            if let item = store.item(itemID) {
                RequestDetail(
                    item: item, draft: store.draft(for: itemID), status: status, now: Date(), isDeviceReady: isDeviceReady, isDeviceChecked: isDeviceChecked,
                    onText: { store.setText($0, for: itemID) },
                    onPick: { store.toggleOption($0, for: itemID) },
                    onDecide: { decide(item) },
                    onComment: { comment() }
                )
                .padding(Theme.gutter)
            } else if store.hasAnswer {
                Text(Ja.Desk.nothingToDecide).font(.body).foregroundStyle(.secondary).wrapping().padding(Theme.gutter)
            } else {
                // Opened from a notification before the desk was read: the request is not missing, it is not read yet.
                SkeletonCard().padding(Theme.gutter)
            }
        }
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        #endif
    }

    private var status: ReplyStatus {
        if store.sending.contains(itemID) { return .sending }
        if let failure = store.sendFailures[itemID] { return .failed(failure) }
        return .idle
    }

    private func decide(_ item: DeskItem) {
        guard let reply = store.draft(for: itemID).decision(for: item) else { return }
        Task { await send(reply) }
    }

    private func comment() {
        let text = store.draft(for: itemID).text
        Task { await send(.comment(text)) }
    }

    /// Decides, and moves on at once to the next item: the node is told from the outbox.
    private func send(_ reply: DeskReply) async {
        let pile = store.items
        await store.send(reply, for: itemID)
        if store.outbox.decidedItemIDs.contains(itemID) { onMoveOn(Desk.next(after: itemID, in: pile)) }
    }
}
#endif
