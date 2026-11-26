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
            .onChange(of: model.router.tab) { model.tabChanged() }
            .onChange(of: model.push.registration) { model.registerPush() }
            .onChange(of: model.push.pendingLink) { model.takeLink() }
            .sheet(isPresented: $model.showsSettings) {
                if let session = model.session {
                    SettingsSheet(
                        host: model.homeNode.host,
                        notifications: session.notifications,
                        nodeSendsPushKinds: session.nodeSendsPushKinds,
                        onChangeHomeNode: model.leave,
                        onResetKey: model.resetKey
                    )
                }
            }
    }
}

extension View {
    /// Adds what every shell has around its screens (see `ShellChrome`).
    func shellChrome(_ model: ShellModel) -> some View {
        modifier(ShellChrome(model: model))
    }
}
