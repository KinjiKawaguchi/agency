#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The costs on their own: what funds the work and how fast it is used, then each team's token use over a period.
public struct CostsScreen: View {
    private let store: CostsStore
    private let onSignInAgain: () -> Void

    /// - Parameter onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(store: CostsStore, onSignInAgain: @escaping () -> Void) {
        self.store = store
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            CostsPage(store: store, onSignInAgain: onSignInAgain).navigationTitle(Ja.Costs.title)
        }
    }
}

/// The costs without a navigation stack around them, so that a screen with several pages can hold them.
struct CostsPage: View {
    let store: CostsStore
    let onSignInAgain: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                banner
                if store.loading == .loaded || !store.teams.isEmpty {
                    CostsContent(
                        funding: store.fundingSummaries(now: Date()), teams: store.teams, days: store.days, now: Date(),
                        onChoose: { days in Task { await store.choose(days: days) } },
                        onTryAgain: { Task { await store.refresh() } }
                    )
                }
            }
            .padding(Theme.gutter)
        }
        .refreshable { await store.refresh() }
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
        } else if store.loading == .loading && store.teams.isEmpty {
            SkeletonCard()
        }
    }
}
#endif
