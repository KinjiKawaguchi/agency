import AppKit

/// Asking Apple for a push token, and naming this device, on the Mac.
struct AppKitPushPlatform: PushPlatform {
    /// A kind of device, not the name the owner gave this Mac.
    var deviceLabel: String { "Mac" }

    func registerForRemoteNotifications() {
        NSApplication.shared.registerForRemoteNotifications()
    }
}
