import AgencyKit
import Observation
import os
import UserNotifications

/// Asks for notification permission at the right moment, registers with Apple, and keeps what the
/// shell needs: the device registration to tell the node, and where a tapped notification leads.
@MainActor
@Observable
final class PushCoordinator {
    private static let deferredAtKey = "pushPromptDeferredAt"
    private static let logger = Logger(subsystem: "agency.app", category: "push")

    /// Set once Apple has given this device a token.
    private(set) var registration: PushRegistration?
    /// Where a tapped notification leads, until the shell takes it.
    private(set) var pendingLink: NotificationLink?
    /// Whether the one-line explanation is on screen.
    var isShowingPrompt = false

    @ObservationIgnored private let platform: any PushPlatform
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private let center: UNUserNotificationCenter
    @ObservationIgnored private var isSignedIn = false

    init(platform: any PushPlatform, defaults: UserDefaults = .standard, center: UNUserNotificationCenter = .current()) {
        self.platform = platform
        self.defaults = defaults
        self.center = center
    }

    /// Called at launch: a permission given earlier needs a fresh token each time.
    func start() {
        Task { await refresh() }
    }

    /// Called when the home node's page has loaded, which means the owner is signed in.
    func noteSignedIn() {
        guard !isSignedIn else { return }
        isSignedIn = true
        Task { await refresh() }
    }

    func open(_ link: NotificationLink) {
        pendingLink = link
    }

    func takePendingLink() -> NotificationLink? {
        defer { pendingLink = nil }
        return pendingLink
    }

    func acceptPrompt() {
        Task {
            let isGranted = (try? await center.requestAuthorization(options: [.alert, .sound, .badge])) ?? false
            if isGranted { platform.registerForRemoteNotifications() }
        }
    }

    func deferPrompt() {
        defaults.set(Date(), forKey: Self.deferredAtKey)
    }

    /// Shows the number of waiting requests on the app icon. Without the badge permission, iOS
    /// ignores it.
    func setBadge(_ count: Int) {
        Task { try? await center.setBadgeCount(count) }
    }

    func didRegister(deviceToken: Data) {
        guard let token = PushToken(deviceToken: deviceToken), let environment = Self.environment else {
            Self.logger.error("Cannot build a push registration: empty token or unknown environment")
            return
        }
        registration = PushRegistration(token: token, environment: environment, label: platform.deviceLabel)
    }

    func didFailToRegister(_ error: Error) {
        Self.logger.error("Registration for remote notifications failed: \(error.localizedDescription, privacy: .public)")
    }

    private func refresh() async {
        let settings = await center.notificationSettings()
        let step = PushPromptPolicy.next(
            authorization: Self.authorization(of: settings),
            isSignedIn: isSignedIn,
            lastDeferredAt: defaults.object(forKey: Self.deferredAtKey) as? Date,
            now: Date()
        )
        switch step {
        case .register: platform.registerForRemoteNotifications()
        case .askForPermission: isShowingPrompt = true
        case .nothing: break
        }
    }

    private static func authorization(of settings: UNNotificationSettings) -> NotificationAuthorization {
        switch settings.authorizationStatus {
        case .notDetermined: .notDetermined
        case .authorized, .provisional, .ephemeral: .authorized
        default: .denied
        }
    }

    /// The APNs environment this build is signed for, from the `aps-environment` entitlement value
    /// that project.yml also puts in Info.plist.
    private static var environment: PushEnvironment? {
        (Bundle.main.object(forInfoDictionaryKey: "AgencyAPNsEnvironment") as? String)
            .flatMap(PushEnvironment.init(apsEnvironment:))
    }
}
