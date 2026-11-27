import AgencyModel
import AgencyUI
import SwiftUI

/// What the news polling runs for: it starts when the app is in front and a session exists, and starts again for
/// a new session (the key was made again). A task that only knew the scene phase would find no session at the
/// first launch, return, and not run again until the app went to the background and back.
private struct NewsWatch: Hashable {
    let phase: ScenePhase
    let session: ObjectIdentifier?
}

/// What every shell has around its screens: the banners over them, the news polled while the app is in
/// front, the settings, and the lifecycle that starts the session and follows the push registration and a
/// tapped notification.
private struct ShellChrome: ViewModifier {
    @Bindable var model: ShellModel
    @Environment(\.scenePhase) private var scenePhase

    func body(content: Content) -> some View {
        content
            .environment(\.keyProtection, .platform)
            .overlay(alignment: .top) {
                if let session = model.session {
                    NewsBannerStack(
                        banners: session.news.banners,
                        onOpen: { model.open($0) },
                        onClose: { session.news.dismiss() }
                    )
                    .animation(.default, value: session.news.banners)
                }
            }
            .task { model.start() }
            .task(id: NewsWatch(phase: scenePhase, session: model.session.map(ObjectIdentifier.init))) {
                if scenePhase == .active { await model.watchNews() }
            }
            .task(id: model.session?.news.banners) { await model.dismissBannersSoon() }
            .onChange(of: model.session?.notifications.preferences.pushKinds) { model.registerPush() }
            // These follow the desk whichever screen is open: the Mac's sidebar keeps only the open screen alive.
            .onChange(of: model.session?.desk.waitingCount) { _, count in if let count { model.nodes.recordWaiting(count, for: model.homeNode) } }
            .onChange(of: model.session?.desk.loading) { _, loading in if let loading { model.deskLoaded(loading) } }
            // Which screens are offered follows what the node says the viewer is.
            .onChange(of: model.session?.desk.standing, initial: true) { _, standing in if let standing { model.router.standing = standing } }
            // A device's own removal that took effect: its key can never sign again, so a new one is made.
            .onChange(of: model.session?.devices?.didRemoveThisDevice) { _, removed in if removed == true { model.renewKeyAfterRemoval() } }
            .onChange(of: model.session?.desk.items) { model.publishWidgets() }
            .onChange(of: model.router.tab) { model.tabChanged() }
            .onChange(of: model.push.registration) { model.registerPush() }
            .onChange(of: model.push.pendingTap) { model.takeLink() }
            .sheet(isPresented: $model.showsSettings) {
                if let session = model.session {
                    SettingsSheet(
                        homeNode: model.homeNode,
                        nodes: model.nodes,
                        notifications: session.notifications,
                        widgets: session.widgets.preferences,
                        nodeSendsPushKinds: session.nodeSendsPushKinds,
                        onSwitchHomeNode: model.switchHomeNode(to:),
                        onStopPush: model.stopPush(at:),
                        onChangeHomeNode: model.leave,
                        onResetKey: model.resetKey
                    )
                }
            }
            .linkNoticeAlert(model)
    }
}

private extension View {
    /// Said once when a tapped notification belongs to a home node this device does not have.
    func linkNoticeAlert(_ model: ShellModel) -> some View {
        alert(
            Ja.HomeNodes.notificationNotOpened,
            isPresented: Binding(get: { model.linkNotice != nil }, set: { if !$0 { model.linkNotice = nil } }),
            presenting: model.linkNotice
        ) { _ in
            Button(Ja.Desk.close) {}
        } message: { text in
            Text(text)
        }
    }
}

extension View {
    /// Adds what every shell has around its screens (see `ShellChrome`).
    func shellChrome(_ model: ShellModel) -> some View {
        modifier(ShellChrome(model: model))
    }
}
