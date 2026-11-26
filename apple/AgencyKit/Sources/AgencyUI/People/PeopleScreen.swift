#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The participants and work tab: every team's participants, what they are doing, the work and the epics.
public struct PeopleScreen: View {
    private let store: PeopleStore
    private let onOpenDesk: () -> Void
    private let onSignInAgain: () -> Void

    /// - Parameters:
    ///   - onOpenDesk: shows the desk, for a participant who waits on the viewer.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(store: PeopleStore, onOpenDesk: @escaping () -> Void, onSignInAgain: @escaping () -> Void) {
        self.store = store
        self.onOpenDesk = onOpenDesk
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            PeoplePage(store: store, onOpenDesk: onOpenDesk, onSignInAgain: onSignInAgain).navigationTitle(Ja.People.title)
        }
    }
}

/// The participants and work without a navigation stack around them, so that a screen with several pages can hold them.
struct PeoplePage: View {
    let store: PeopleStore
    let onOpenDesk: () -> Void
    let onSignInAgain: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                banner
                if store.hasAnswered || !store.sections.isEmpty {
                    PeopleContent(sections: store.sections, failures: store.failures, now: Date(), onOpenDesk: onOpenDesk,
                                  onTryAgain: { Task { await store.refresh() } })
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
        } else if !store.hasAnswered && (store.loading == .loading || store.loading == .idle) && store.sections.isEmpty {
            ForEach(0..<3, id: \.self) { _ in SkeletonCard() }
        }
    }
}
#endif
