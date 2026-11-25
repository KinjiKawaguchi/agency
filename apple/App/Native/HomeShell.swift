import AgencyKit
import AgencyModel
import AgencyUI
import SwiftUI

/// The app for one home node: the desk as native screens, and the screens that are still web pages
/// behind the other tabs. One web view serves all of them and shows the page of the tab selected.
struct HomeShell: View {
    let homeNode: HomeNodeURL
    let push: PushCoordinator
    let onChangeHomeNode: () -> Void

    enum Tab: Hashable {
        case desk, people, org, costs, devices

        var path: String? {
            switch self {
            case .desk: nil
            case .people: "/people"
            case .org: "/org"
            case .costs: "/costs"
            case .devices: "/devices"
            }
        }
    }

    @State private var session: NativeSession?
    @State private var flow: AccessSignInFlow?
    @State private var web: NodeWebModel?
    @State private var tab: Tab = .desk
    @State private var openItemID: String?
    @State private var showsSettings = false
    @State private var triedSignIn = false

    var body: some View {
        Group {
            if let session {
                TabView(selection: $tab) {
                    desk(session)
                        .tabItem { Label(Ja.Desk.title, systemImage: "tray.full") }
                        .tag(Tab.desk)
                    webTab(.people, "人と仕事", "person.2")
                    webTab(.org, "組織図", "rectangle.3.group")
                    webTab(.costs, "費用", "yensign.circle")
                    webTab(.devices, "端末", "iphone")
                }
                .sheet(isPresented: $showsSettings) {
                    SettingsSheet(
                        host: homeNode.host,
                        onChangeHomeNode: {
                            showsSettings = false
                            Task { await leave() }
                        },
                        onResetKey: resetKey
                    )
                }
            } else {
                ProgressView()
            }
        }
        .task { start() }
        .onChange(of: tab) { _, tab in openWeb(tab) }
        .onChange(of: push.registration) { _, registration in
            web?.updatePush(registration)
            if let registration, let session { Task { await session.registerPush(registration) } }
        }
        .onChange(of: push.pendingLink) { takeLink() }
    }

    // MARK: Tabs

    private func desk(_ session: NativeSession) -> some View {
        DeskScreen(
            store: session.desk, pairing: session.pairing, notice: notice(session), openItemID: $openItemID,
            onSignInAgain: { Task { await session.signInAgain() } },
            onSettings: { showsSettings = true }
        )
        .onChange(of: session.desk.waitingCount) { _, count in push.setBadge(count) }
        .onChange(of: session.desk.loading) { _, loading in loaded(loading, session) }
    }

    @ViewBuilder
    private func webTab(_ tag: Tab, _ title: String, _ symbol: String) -> some View {
        Group {
            if tab == tag, let web {
                WebTabView(model: web, onChangeHomeNode: { Task { await leave() } })
            } else {
                Color.clear
            }
        }
        .tabItem { Label(title, systemImage: symbol) }
        .tag(tag)
    }

    private func notice(_ session: NativeSession) -> String? {
        [session.keyProblem.map { _ in Ja.Device.noKey }, session.signInProblem].compactMap { $0 }.first
    }

    // MARK: Behaviour

    private func start() {
        guard session == nil else { return }
        let tokens = AccessTokenStore(host: homeNode.host)
        let flow = AccessSignInFlow(homeNode: homeNode, tokens: tokens)
        self.flow = flow
        session = NativeSession(homeNode: homeNode, tokens: tokens, signIn: flow)
        takeLink()
    }

    private func openWeb(_ tab: Tab) {
        guard let path = tab.path, let flow else { return }
        if let web {
            web.openPath(path)
        } else {
            web = NodeWebModel(homeNode: homeNode, push: push.registration, accessSignIn: flow, path: path)
        }
    }

    /// A notification was tapped: open the request on the desk.
    private func takeLink() {
        guard session != nil, let link = push.takePendingLink() else { return }
        tab = .desk
        openItemID = DeskItem.key(org: link.org, team: link.team, request: link.request)
    }

    private func loaded(_ loading: DeskStore.Loading, _ session: NativeSession) {
        switch loading {
        case .loaded:
            push.noteSignedIn()
            if let registration = push.registration { Task { await session.registerPush(registration) } }
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

    /// Makes this phone's key again, for when Face ID was changed and the old key can no longer sign.
    private func resetKey() {
        SecureEnclaveSigner.deleteKey(host: homeNode.host)
        showsSettings = false
        session = nil
        start()
    }

    /// The number on the icon and the sign-in belong to the node that is being left.
    private func leave() async {
        push.setBadge(0)
        await flow?.forget()
        onChangeHomeNode()
    }
}
