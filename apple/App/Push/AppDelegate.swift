import AgencyKit
import UIKit
import UserNotifications

/// Receives what SwiftUI's lifecycle does not: the device token, and taps on notifications.
final class AppDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    let push = PushCoordinator(platform: UIKitPushPlatform())

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        push.start()
        return true
    }

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        push.didRegister(deviceToken: deviceToken)
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        push.didFailToRegister(error)
    }

    /// A push that asks the app to read the desk while it is closed (`content-available`): the widgets show the
    /// result. The app reads nothing when it is in front, where the screens read for themselves.
    func application(_ application: UIApplication, didReceiveRemoteNotification userInfo: [AnyHashable: Any]) async -> UIBackgroundFetchResult {
        guard DeskWake.isWake(userInfo), application.applicationState != .active else { return .noData }
        switch await BackgroundDeskRefresh.run() {
        case .written?: return .newData
        case .notRead?: return .failed
        case nil: return .noData
        }
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
        guard let tap = NotificationTap(userInfo: response.notification.request.content.userInfo) else { return }
        await push.open(tap)
    }
}
