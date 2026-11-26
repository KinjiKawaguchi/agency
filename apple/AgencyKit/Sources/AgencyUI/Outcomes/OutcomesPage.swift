#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// The outcomes without a navigation stack around them, so that the results screen can hold them as a page.
///
/// The numbers are business data. On the iPhone a plain cover goes over the page whenever the app is not in front, so
/// that what the system keeps of it for the app switcher shows none of them.
struct OutcomesPage: View {
    let store: OutcomesStore
    let onSignInAgain: () -> Void
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                banner
                if store.hasAnswered {
                    OutcomesContent(
                        orgs: store.orgs, days: store.days,
                        onChoose: { days in Task { await store.choose(days: days) } },
                        onTryAgain: { Task { await store.refresh() } }
                    )
                }
            }
            .padding(Theme.gutter)
        }
        .refreshable { await store.refresh() }
        .polling { await store.refresh() }
        .overlay { if isCovered { OutcomesCover() } }
    }

    private var isCovered: Bool {
        #if os(iOS)
        scenePhase != .active
        #else
        false
        #endif
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
        } else if !store.hasAnswered {
            SkeletonCard()
        }
    }
}

/// A plain page over the numbers: the name of the page and a lock, nothing of them.
private struct OutcomesCover: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "lock").font(.largeTitle).accessibilityHidden(true)
            Text(Ja.Outcomes.title).font(.headline)
        }
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.background)
    }
}
#endif
