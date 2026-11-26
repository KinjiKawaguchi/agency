#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The organization tab: the org chart and the participants and work, behind a switch at the top, as the web client groups them.
public struct OrganizationScreen: View {
    private let orgChart: OrgChartStore
    private let people: PeopleStore
    private let onOpenDesk: () -> Void
    private let onOpenOverview: () -> Void
    private let onSignInAgain: () -> Void
    @State private var page: Page = .orgChart

    enum Page: CaseIterable, Hashable {
        case orgChart, people

        var title: String {
            switch self {
            case .orgChart: Ja.Tabs.orgChart
            case .people: Ja.Tabs.people
            }
        }
    }

    /// - Parameters:
    ///   - onOpenDesk: shows the desk, for a team or a participant that waits on the viewer.
    ///   - onOpenOverview: shows the overview, for a team the chart opens.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(
        orgChart: OrgChartStore, people: PeopleStore, onOpenDesk: @escaping () -> Void,
        onOpenOverview: @escaping () -> Void, onSignInAgain: @escaping () -> Void
    ) {
        self.orgChart = orgChart
        self.people = people
        self.onOpenDesk = onOpenDesk
        self.onOpenOverview = onOpenOverview
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker(Ja.Tabs.organization, selection: $page) {
                    ForEach(Page.allCases, id: \.self) { Text($0.title).tag($0) }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, Theme.gutter)
                .padding(.vertical, 8)
                switch page {
                case .orgChart:
                    OrgChartPage(store: orgChart, onOpenDesk: onOpenDesk, onOpenOverview: onOpenOverview, onSignInAgain: onSignInAgain)
                case .people:
                    PeoplePage(store: people, onOpenDesk: onOpenDesk, onSignInAgain: onSignInAgain)
                }
            }
            .navigationTitle(Ja.Tabs.organization)
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
        }
    }
}
#endif
