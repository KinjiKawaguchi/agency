import UIKit

/// Asking Apple for a push token, and naming this device, on the iPhone.
struct UIKitPushPlatform: PushPlatform {
    var deviceLabel: String { UIDevice.current.model }

    func registerForRemoteNotifications() {
        UIApplication.shared.registerForRemoteNotifications()
    }
}
