#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The overview tab: every org and team the account sees, and what funds the work.
public struct OverviewScreen: View {
    private let store: OverviewStore
    private let onOpenDesk: () -> Void
    private let onSignInAgain: () -> Void

    /// - Parameters:
    ///   - onOpenDesk: shows the desk, for a team with requests waiting on the viewer.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(store: OverviewStore, onOpenDesk: @escaping () -> Void, onSignInAgain: @escaping () -> Void) {
        self.store = store
        self.onOpenDesk = onOpenDesk
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.spacing) {
                    banner
                    if !store.orgs.isEmpty || store.loading == .loaded {
                        OverviewContent(
                            sections: store.sections, waiting: store.waiting, funding: store.fundingSummaries(now: Date()),
                            cost: store.weekCost(of:), now: Date(), onOpenDesk: onOpenDesk,
                            onTryAgain: { Task { await store.refresh() } }
                        )
                    }
                }
                .padding(Theme.gutter)
            }
            .refreshable { await store.refresh() }
            .navigationTitle(Ja.Overview.title)
        }
        .polling { await store.refresh() }
    }

    @ViewBuilder
    private var banner: some View {
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
        } else if store.loading == .loading && store.orgs.isEmpty {
            ForEach(0..<3, id: \.self) { _ in SkeletonCard() }
        }
    }
}
#endif
