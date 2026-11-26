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
    /// The KPI a number is being entered for, and the org it is in.
    @State private var entering: Entering?

    private struct Entering: Identifiable {
        let org: String
        let today: String
        let card: KpiCard
        var id: String { "\(org)/\(card.id)" }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                banner
                if store.hasAnswered {
                    OutcomesContent(
                        orgs: store.orgs, days: store.days, histories: store.histories, costs: store.costs,
                        onChoose: { days in Task { await store.choose(days: days) } },
                        onTryAgain: { Task { await store.refresh() } },
                        onEnter: { org, card in entering = Entering(org: org.id, today: org.until, card: card) },
                        onToggleHistory: { org, card in toggleHistory(org: org.id, card: card) }
                    )
                }
            }
            .padding(Theme.gutter)
        }
        .refreshable { await store.refresh() }
        .polling { await store.refresh() }
        .coveredWhenInactive()
        .sheet(item: $entering) { target in
            EnterNumberSheet(card: target.card, today: target.today) { form in
                await store.enter(org: target.org, kpi: target.card.kpi, form: form)
            }
        }
    }

    private func toggleHistory(org: String, card: KpiCard) {
        if store.histories[OutcomesStore.historyKey(org: org, kpi: card.id)] == nil {
            Task { await store.showHistory(org: org, kpi: card.kpi) }
        } else {
            store.hideHistory(org: org, kpi: card.id)
        }
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

extension View {
    /// A plain cover over the numbers whenever the iPhone app is not in front, so that what the system keeps of the screen for the
    /// app switcher shows none of them. The Mac keeps no such picture and draws none.
    func coveredWhenInactive() -> some View { modifier(InactiveCover()) }
}

private struct InactiveCover: ViewModifier {
    @Environment(\.scenePhase) private var scenePhase

    func body(content: Content) -> some View {
        content.overlay { if isCovered { OutcomesCover() } }
    }

    private var isCovered: Bool {
        #if os(iOS)
        scenePhase != .active
        #else
        false
        #endif
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
