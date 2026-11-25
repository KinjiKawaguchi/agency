#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The desk tab: the pile, a request open, and the banners that say what is wrong.
public struct DeskScreen: View {
    private let store: DeskStore
    private let onSignInAgain: () -> Void

    /// - Parameters:
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(store: DeskStore, onSignInAgain: @escaping () -> Void) {
        self.store = store
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.spacing) {
                    banners
                    DeskPile(items: store.items, situation: store.situation, sent: Set(store.sent.keys), now: Date())
                    if !store.recent.isEmpty || store.recentFailure != nil {
                        RecentDecisions(entries: store.recent, failure: store.recentFailure, now: Date()).padding(.top, 16)
                    }
                }
                .padding(Theme.gutter)
            }
            .refreshable { await store.refresh() }
            .navigationTitle(Ja.Desk.title)
            .navigationDestination(for: String.self) { id in
                RequestDetailScreen(store: store, itemID: id)
            }
        }
        .task { await store.refresh() }
    }

    @ViewBuilder
    private var banners: some View {
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
    let itemID: String

    var body: some View {
        ScrollView {
            if let item = store.item(itemID) {
                RequestDetail(
                    item: item, draft: store.draft(for: itemID), status: status, now: Date(),
                    onText: { store.setText($0, for: itemID) },
                    onPick: { store.toggleOption($0, for: itemID) },
                    onDecide: { decide(item) },
                    onComment: { comment() }
                )
                .padding(Theme.gutter)
            } else {
                Text(Ja.Desk.nothingToDecide).font(.body).foregroundStyle(.secondary).wrapping().padding(Theme.gutter)
            }
        }
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        #endif
    }

    private var status: ReplyStatus {
        if store.sending.contains(itemID) { return .sending }
        if let sent = store.sent[itemID] { return .sent(sent) }
        if let failure = store.sendFailures[itemID] { return .failed(failure) }
        return .idle
    }

    private func decide(_ item: DeskItem) {
        guard let reply = store.draft(for: itemID).decision(for: item) else { return }
        Task { await store.send(reply, for: itemID) }
    }

    private func comment() {
        let text = store.draft(for: itemID).text
        Task { await store.send(.comment(text), for: itemID) }
    }
}
#endif
