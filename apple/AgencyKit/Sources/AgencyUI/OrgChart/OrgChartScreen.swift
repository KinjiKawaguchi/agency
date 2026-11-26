#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The org chart on its own: every org of the account under the account, or one org.
public struct OrgChartScreen: View {
    private let store: OrgChartStore
    private let onOpenDesk: () -> Void
    private let onOpenOverview: () -> Void
    private let onSignInAgain: () -> Void

    /// - Parameters:
    ///   - onOpenDesk: shows the desk, for a team with requests waiting on the viewer.
    ///   - onOpenOverview: shows the overview, for any other team.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(store: OrgChartStore, onOpenDesk: @escaping () -> Void, onOpenOverview: @escaping () -> Void, onSignInAgain: @escaping () -> Void) {
        self.store = store
        self.onOpenDesk = onOpenDesk
        self.onOpenOverview = onOpenOverview
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            OrgChartPage(store: store, onOpenDesk: onOpenDesk, onOpenOverview: onOpenOverview, onSignInAgain: onSignInAgain)
                .navigationTitle(Ja.OrgChart.title)
        }
    }
}

/// The org chart without a navigation stack around it, so that a screen with several pages can hold it.
struct OrgChartPage: View {
    let store: OrgChartStore
    let onOpenDesk: () -> Void
    let onOpenOverview: () -> Void
    let onSignInAgain: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.spacing) {
                banner
                if store.loading == .loaded || !store.rows.isEmpty || !store.orgs.isEmpty {
                    OrgChartContent(
                        title: store.title, orgs: store.orgs, isAll: store.isAll, selected: store.org?.id,
                        rows: store.rows, isFocused: store.focus != nil, isEmptyDepartment: store.isEmptyDepartment,
                        isOrgUnavailable: store.isOrgUnavailable, isOrgLoading: store.isOrgLoading,
                        onChoose: { store.choose($0) }, onFocus: { store.focus(on: $0) }, onWhole: { store.focus(on: nil) },
                        onOpenTeam: { node in (node.signals?.mine ?? 0) > 0 ? onOpenDesk() : onOpenOverview() },
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
            FailedRegion(what: Ja.OrgChart.title, why: reason) { Task { await store.refresh() } }.card()
        } else if (store.loading == .loading || store.loading == .idle) && store.orgs.isEmpty {
            ForEach(0..<3, id: \.self) { _ in SkeletonCard() }
        }
    }
}

/// The choice of org and the rows of the tree. The content only.
struct OrgChartContent: View {
    let title: String
    let orgs: [OrgRef]
    let isAll: Bool
    let selected: String?
    let rows: [OrgChartRow]
    var isFocused = false
    var isEmptyDepartment = false
    var isOrgUnavailable = false
    var isOrgLoading = false
    var onChoose: ((String) -> Void)?
    var onFocus: ((String) -> Void)?
    var onWhole: (() -> Void)?
    var onOpenTeam: ((TeamNode) -> Void)?
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.title3.weight(.semibold)).wrapping()
            if orgs.isEmpty {
                Text(Ja.OrgChart.noOrgs).font(.subheadline).foregroundStyle(.secondary).wrapping()
            } else {
                choice
                if isFocused { Button(Ja.OrgChart.whole) { onWhole?() }.font(.subheadline) }
                if isOrgUnavailable {
                    FailedRegion(what: Ja.OrgChart.title, why: Ja.OrgChart.orgUnavailable, onTryAgain: onTryAgain)
                } else if isOrgLoading {
                    ForEach(0..<3, id: \.self) { _ in SkeletonCard() }
                }
                ForEach(rows) { row in
                    OrgChartRowView(row: row, onChooseOrg: onChoose, onFocus: onFocus, onOpenTeam: onOpenTeam)
                }
                if isEmptyDepartment { Text(Ja.OrgChart.emptyDepartment).font(.subheadline).foregroundStyle(.secondary).wrapping() }
            }
        }
    }

    /// The organization menu, with "all" first when there is more than one.
    @ViewBuilder
    private var choice: some View {
        if orgs.count > 1 {
            VStack(alignment: .leading, spacing: 6) {
                Text(Ja.OrgChart.org).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                FlowLine(spacing: 8) {
                    Button(Ja.OrgChart.allOrgs) { onChoose?(OrgChartStore.all) }.buttonStyle(ChoiceStyle(isSelected: isAll))
                    ForEach(orgs, id: \.id) { one in
                        Button(one.label) { onChoose?(one.id) }.buttonStyle(ChoiceStyle(isSelected: !isAll && one.id == selected))
                    }
                }
            }
        }
    }
}
#endif
