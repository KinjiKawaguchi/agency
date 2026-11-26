import AgencyKit
import AgencyModel
import AgencyUI
import Foundation
import Observation

/// What the app does for one home node, whatever the platform draws it with: the session, the screen that is
/// open, the sign-in, what a banner or a tapped notification opens, and the news polled while the app is in
/// front. The shells (a tab bar on the iPhone, a sidebar on the Mac) only show it.
@MainActor
@Observable
final class ShellModel {
    private(set) var session: NativeSession?
    var tab: AppTab = .desk
    var organizationPage: OrganizationPage = .orgChart
    var openItemID: String?
    var showsSettings = false

    let homeNode: HomeNodeURL
    @ObservationIgnored let push: PushCoordinator
    @ObservationIgnored private let deviceLabel: String
    @ObservationIgnored private let onChangeHomeNode: () -> Void
    @ObservationIgnored private var flow: AccessSignInFlow?
    @ObservationIgnored private var triedSignIn = false

    init(homeNode: HomeNodeURL, push: PushCoordinator, deviceLabel: String, onChangeHomeNode: @escaping () -> Void) {
        self.homeNode = homeNode
        self.push = push
        self.deviceLabel = deviceLabel
        self.onChangeHomeNode = onChangeHomeNode
    }

    /// What the desk says is wrong with this device's key or with the sign-in, if anything.
    var notice: String? {
        guard let session else { return nil }
        return [session.keyProblem.map { _ in Ja.Device.noKey }, session.signInProblem].compactMap { $0 }.first
    }

    // MARK: Start

    func start() {
        guard session == nil else { return }
        let tokens = AccessTokenStore(host: homeNode.host)
        let flow = AccessSignInFlow(homeNode: homeNode, tokens: tokens)
        self.flow = flow
        session = NativeSession(homeNode: homeNode, tokens: tokens, signIn: flow, deviceLabel: deviceLabel)
        takeLink()
    }

    /// The desk was read, or could not be.
    func deskLoaded(_ loading: DeskStore.Loading) {
        guard let session else { return }
        switch loading {
        case .loaded:
            push.noteSignedIn()
            registerPush()
        case .failed:
            // The first launch has no token: ask for the sign-in once, without waiting for a tap.
            if session.desk.needsSignIn, !triedSignIn {
                triedSignIn = true
                Task { await session.signInAgain() }
            }
        default:
            break
        }
    }

    /// Tells the node this device's push token and the kinds the owner chose, when there is a token.
    func registerPush() {
        guard let session, let registration = push.registration else { return }
        Task { await session.registerPush(registration) }
    }

    // MARK: Where things lead

    /// A banner was tapped: go where it leads.
    func open(_ banner: NewsBanner) {
        session?.news.dismiss()
        switch banner.destination {
        case .desk(let itemID):
            tab = .desk
            openItemID = itemID
        case .people:
            organizationPage = .people
            tab = .organization
        case .devices:
            tab = .devices
        }
    }

    /// A notification was tapped: go where it leads.
    func takeLink() {
        guard session != nil, let link = push.takePendingLink() else { return }
        switch link {
        case .request(let request):
            tab = .desk
            openItemID = DeskItem.key(org: request.org, team: request.team, request: request.request)
        case .devices:
            tab = .devices
        case .decision:
            tab = .desk
        }
    }

    // MARK: News

    /// Reads the news while the app is in front: now, and every 30 seconds.
    func watchNews() async {
        guard let session else { return }
        while !Task.isCancelled {
            await session.news.poll()
            try? await Task.sleep(for: .seconds(30))
        }
    }

    /// A banner stays a few seconds, long enough to read and not long enough to be in the way.
    func dismissBannersSoon() async {
        guard let session, !session.news.banners.isEmpty else { return }
        try? await Task.sleep(for: .seconds(7))
        if !Task.isCancelled { session.news.dismiss() }
    }

    /// The devices screen shows a device that asks to be added by itself, so it makes no banner there.
    func tabChanged() {
        session?.news.suppressedKinds = tab == .devices ? [.device] : []
    }

    // MARK: Settings

    /// Makes this device's key again, for when the biometrics were changed and the old key can no longer sign.
    func resetKey() {
        SecureEnclaveSigner.deleteKey(host: homeNode.host)
        showsSettings = false
        session = nil
        start()
    }

    /// The number on the icon and the sign-in belong to the node that is being left.
    func leave() {
        showsSettings = false
        push.setBadge(0)
        flow?.forget()
        onChangeHomeNode()
    }
}
