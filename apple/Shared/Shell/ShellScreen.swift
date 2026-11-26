import AgencyModel
import AgencyUI
import SwiftUI

/// The screen of a tab, the same on every platform.
struct ShellScreen: View {
    let tab: AppTab
    let model: ShellModel
    let session: NativeSession

    var body: some View {
        switch tab {
        case .desk:
            DeskScreen(
                store: session.desk, pairing: session.pairing, notice: model.notice, openItemID: Bindable(model.router).openItemID,
                onSignInAgain: signInAgain, onSettings: { model.showsSettings = true }
            )
            .onChange(of: session.desk.waitingCount) { _, count in model.push.setBadge(count) }
            .onChange(of: session.desk.loading) { _, loading in model.deskLoaded(loading) }
        case .overview:
            OverviewScreen(store: session.overview, onOpenDesk: { model.router.tab = .desk }, onSignInAgain: signInAgain)
        case .organization:
            OrganizationScreen(
                orgChart: session.orgChart, people: session.people, page: Bindable(model.router).organizationPage,
                onOpenDesk: { model.router.tab = .desk }, onOpenOverview: { model.router.tab = .overview }, onSignInAgain: signInAgain
            )
        case .results:
            ResultsScreen(costs: session.costs, evaluation: session.evaluation, onSignInAgain: signInAgain)
        case .devices:
            DevicesScreen(store: session.devices, pairing: session.pairing, notice: session.keyProblem.map { _ in Ja.Device.noKey })
        }
    }

    private func signInAgain() {
        Task { await session.signInAgain() }
    }
}
