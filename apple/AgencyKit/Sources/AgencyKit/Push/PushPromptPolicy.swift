import Foundation

public enum NotificationAuthorization: Equatable, Sendable {
    case notDetermined
    case denied
    case authorized
}

/// What the app does about notifications right now.
public enum PushStep: Equatable, Sendable {
    /// Ask Apple for a device token.
    case register
    /// Explain why, then ask the owner for permission.
    case askForPermission
    case nothing
}

/// Decides when the app asks for notification permission: not before the owner has signed in, and
/// after a "Not now" only once a while has passed.
public enum PushPromptPolicy {
    public static let retryInterval: TimeInterval = 7 * 24 * 60 * 60

    public static func next(
        authorization: NotificationAuthorization,
        isSignedIn: Bool,
        lastDeferredAt: Date?,
        now: Date
    ) -> PushStep {
        switch authorization {
        case .authorized:
            return .register
        case .denied:
            return .nothing
        case .notDetermined:
            guard isSignedIn else { return .nothing }
            if let lastDeferredAt, now.timeIntervalSince(lastDeferredAt) < retryInterval { return .nothing }
            return .askForPermission
        }
    }
}
