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
    /// Which screen is open, and where a banner or a notification leads.
    let router = ShellRouter()
    var showsSettings = false
    /// Said once when a tapped notification belongs to a home node this device does not have.
    var linkNotice: String?
    /// Whether the account has been read once (with an answer or without), after which a node's id is as settled as it will be.
    private var hasReadAccount = false

    let homeNode: HomeNodeURL
    @ObservationIgnored let push: PushCoordinator
    @ObservationIgnored private let deviceLabel: String
    /// Who holds the home nodes: the shell asks it to switch, leave, add and forget.
    let nodes: any HomeNodeSwitching
    @ObservationIgnored private var flow: AccessSignInFlow?
    @ObservationIgnored private var triedSignIn = false

    init(homeNode: HomeNodeURL, push: PushCoordinator, deviceLabel: String, nodes: any HomeNodeSwitching) {
        self.homeNode = homeNode
        self.push = push
        self.deviceLabel = deviceLabel
        self.nodes = nodes
    }

    /// What the desk says is wrong with this device's key or with the sign-in, if anything.
    var notice: String? {
        guard let session else { return nil }
        return [session.keyProblem.map { _ in Ja.Key.noKey(.platform) }, session.signInProblem].compactMap { $0 }.first
    }

    // MARK: Start

    /// - Parameter standing: what the node said the viewer is, when this session replaces one of the same node and account.
    func start(standing: Standing = .loading) {
        guard session == nil else { return }
        let tokens = AccessTokenStore(host: homeNode.host)
        let flow = AccessSignInFlow(homeNode: homeNode, tokens: tokens)
        self.flow = flow
        session = NativeSession(homeNode: homeNode, tokens: tokens, signIn: flow, deviceLabel: deviceLabel, standing: standing)
        takeLink()
    }

    /// The desk was read, or could not be.
    func deskLoaded(_ loading: DeskStore.Loading) {
        guard let session else { return }
        switch loading {
        case .loaded:
            push.noteSignedIn()
            registerPush()
            publishWidgets()
            noteHomeNodes()
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

    /// The desk's requests changed, or were read: the widgets show them.
    func publishWidgets() {
        guard let session else { return }
        Task { await session.publishWidgets() }
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
        router.open(banner)
    }

    /// A notification was tapped: go where it leads, on the node that sent it. A tap that names another node of this device goes
    /// to that node first (clearing the icon number and the widgets of this one, as a switch by hand does) and stays pending until
    /// its shell is up, which opens it; one that names a node this device does not have opens nothing of it and says so. While the
    /// open node has not yet said its own id, a tap that is none of the others' waits for the account read, and a tap that has
    /// waited longer than a minute is dropped.
    func takeLink(now: Date = Date()) {
        guard session != nil, let tap = push.pendingTap else { return }
        guard !tap.isStale(now: now) else {
            push.takePendingTap()
            return
        }
        let others = nodes.others.map(\.url)
        let action = NotificationRouting.action(
            forNode: tap.node, current: homeNode, others: others, idOf: nodes.nodeID(of:),
            hasCurrentSaid: nodes.hasSaidID(of: homeNode), isAccountRead: hasReadAccount
        )
        switch action {
        case .openHere:
            push.takePendingTap()
            router.open(tap.link)
        case .switchTo(let node):
            switchHomeNode(to: node)
        case .waitForAccount:
            break
        case .unknownNode:
            push.takePendingTap()
            linkNotice = Ja.HomeNodes.notificationOfAnotherNode(org: tap.org)
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
        session?.news.suppressedKinds = router.suppressedNewsKinds
    }

    // MARK: Settings

    /// Makes this device's key again, for when the biometrics were changed and the old key can no longer sign.
    func resetKey() {
        SecureEnclaveSigner.deleteKey(host: homeNode.host)
        showsSettings = false
        // This device is no longer trusted with the home node: nothing of its desk stays on a widget.
        session?.widgets.clear()
        // The same node and the same account: what the node said the viewer is stays, so that an owner's open screen is not closed to the desk
        // while the new session asks again.
        let standing = session?.desk.standing ?? .loading
        session = nil
        start(standing: standing)
    }

    /// Asks the node to stop sending to this device's push token, before the sign-in goes: the node's alerts would otherwise
    /// keep arriving and set the number on the icon. `false` when it could not be told.
    func stopPush(at node: HomeNodeURL) async -> Bool {
        await PushUnregistration.stop(token: push.registration?.token, at: node)
    }

    /// The sign-in, the widgets and this node's count on the icon belong to the node that is being left.
    func leave() {
        showsSettings = false
        session?.widgets.clear()
        flow?.forget()
        nodes.leave()
    }

    // MARK: The owner's other home nodes

    /// Goes to another home node. Its sign-in, key and choices are as they were; what this node showed on a widget is not the
    /// other node's. The icon keeps the sum over the nodes: this node's count stays in it until the node says another.
    func switchHomeNode(to node: HomeNodeURL) {
        showsSettings = false
        session?.widgets.clear()
        nodes.switchTo(node)
    }

    /// Writes down what the node says about the owner's other nodes, once the desk has been read: it is how the list shows
    /// them (the owner's own entries are the device's).
    private func noteHomeNodes() {
        guard let session else { return }
        let nodes = nodes
        let homeNode = homeNode
        Task {
            let me = await session.readAccount()
            if let me {
                nodes.note(configured: me.homeNodes)
                nodes.note(nodeID: me.nodeId, of: homeNode)
                // A node known by an address other than its own changes id here, and the count recorded under the worked-out id is gone.
                if let count = session.desk.knownWaitingCount { nodes.recordWaiting(count, for: homeNode) }
            }
            // Finished, with an answer or without: a tap that waited for it is placed now, or said not to be of this device.
            hasReadAccount = true
            takeLink()
        }
    }
}
