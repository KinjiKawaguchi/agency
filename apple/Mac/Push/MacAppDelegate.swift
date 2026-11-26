import AgencyKit
import AppKit
import UserNotifications

/// Receives what SwiftUI's lifecycle does not: the device token, and taps on notifications.
@MainActor
final class MacAppDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    let push = PushCoordinator(platform: AppKitPushPlatform())
    let model: MacAppModel

    override init() {
        model = MacAppModel(push: push)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        UNUserNotificationCenter.current().delegate = self
        push.start()
        model.begin()
    }

    /// Closing the window leaves the app running in the menu bar, where its item shows what waits.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func application(_ application: NSApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        push.didRegister(deviceToken: deviceToken)
    }

    func application(_ application: NSApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        push.didFailToRegister(error)
    }

    /// A notification that arrives while the app is open still shows as a banner and sets the badge.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .list, .badge]
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        guard let link = NotificationLink(userInfo: response.notification.request.content.userInfo) else { return }
        await push.open(link)
    }
}
