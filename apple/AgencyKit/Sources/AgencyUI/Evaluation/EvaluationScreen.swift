#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The evaluation on its own: how fast each team of an org ships, over a period the person chooses.
public struct EvaluationScreen: View {
    private let store: EvaluationStore
    private let onSignInAgain: () -> Void

    /// - Parameter onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(store: EvaluationStore, onSignInAgain: @escaping () -> Void) {
        self.store = store
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            EvaluationPage(store: store, onSignInAgain: onSignInAgain).navigationTitle(Ja.Evaluation.title)
        }
    }
}

/// The evaluation without a navigation stack around it, so that a screen with several pages can hold it.
struct EvaluationPage: View {
    let store: EvaluationStore
    let onSignInAgain: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                banner
                if store.hasAnswered || !store.orgs.isEmpty {
                    EvaluationContent(
                        orgs: store.orgs, org: store.org, days: store.days, state: store.state, effort: store.effort,
                        onChooseOrg: { id in Task { await store.choose(org: id) } },
                        onChooseDays: { days in Task { await store.choose(days: days) } },
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
        } else if !store.hasAnswered && (store.loading == .loading || store.loading == .idle) && store.orgs.isEmpty {
            SkeletonCard()
        }
    }
}
#endif
