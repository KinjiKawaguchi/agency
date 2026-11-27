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
        case .overview:
            OverviewScreen(store: session.overview, onOpenDesk: { model.router.tab = .desk }, onSignInAgain: signInAgain)
        case .organization:
            OrganizationScreen(
                orgChart: session.orgChart, people: session.people, page: Bindable(model.router).organizationPage,
                onOpenDesk: { model.router.tab = .desk }, onOpenOverview: { model.router.tab = .overview }, onSignInAgain: signInAgain
            )
        case .results:
            ResultsScreen(costs: session.costs, evaluation: session.evaluation, outcomes: session.outcomes, onSignInAgain: signInAgain)
        case .devices:
            DevicesScreen(
                store: session.devices, pairing: session.pairing, notice: devicesNotice,
                noticeIsFailure: !(model.hasRenewedKeyAfterRemoval && session.keyProblem == nil), pushDelivery: session.pushDelivery,
                keyUnusable: session.keyUnusable.map { KeyUnusableSupport(problem: $0, onTryAgain: model.retryKey, onMakeNew: model.resetKey) }
            )
        }
    }

    /// Something to say above the devices: that this device's key was replaced after its own removal and is not added yet, or why there is
    /// no key (which, after a removal, is that the new key could not be made).
    private var devicesNotice: String? {
        if model.hasRenewedKeyAfterRemoval {
            return session.keyProblem.map { _ in Ja.Devices.Remove.keyNotMade(Ja.Key.noKey(.platform)) } ?? Ja.Devices.Remove.thisDeviceRemoved
        }
        // A stored key that cannot be used is said by its own card, above.
        return session.keyProblem.map { _ in Ja.Key.noKey(.platform) }
    }

    private func signInAgain() {
        Task { await session.signInAgain() }
    }
}
