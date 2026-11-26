#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The two pages of the organization tab.
public enum OrganizationPage: CaseIterable, Hashable, Sendable {
    case orgChart, people

    var title: String {
        switch self {
        case .orgChart: Ja.Tabs.orgChart
        case .people: Ja.Tabs.people
        }
    }
}

/// The organization tab: the org chart and the participants and work, behind a switch at the top, as the web client groups them.
public struct OrganizationScreen: View {
    private let orgChart: OrgChartStore
    private let people: PeopleStore
    private let onOpenDesk: () -> Void
    private let onOpenOverview: () -> Void
    private let onSignInAgain: () -> Void
    @Binding private var page: OrganizationPage

    /// - Parameters:
    ///   - page: the page shown; the shell sets it, for a banner that leads to the participants.
    ///   - onOpenDesk: shows the desk, for a team or a participant that waits on the viewer.
    ///   - onOpenOverview: shows the overview, for a team the chart opens.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(
        orgChart: OrgChartStore, people: PeopleStore, page: Binding<OrganizationPage>, onOpenDesk: @escaping () -> Void,
        onOpenOverview: @escaping () -> Void, onSignInAgain: @escaping () -> Void
    ) {
        self.orgChart = orgChart
        self.people = people
        _page = page
        self.onOpenDesk = onOpenDesk
        self.onOpenOverview = onOpenOverview
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker(Ja.Tabs.organization, selection: $page) {
                    ForEach(OrganizationPage.allCases, id: \.self) { Text($0.title).tag($0) }
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
