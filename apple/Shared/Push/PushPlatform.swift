import Foundation

/// What differs between the platforms in asking Apple for a push token and in naming this device.
@MainActor
protocol PushPlatform {
    /// The kind of device the push token is for, shown in the node's list of tokens (`iPhone`, `Mac`): the
    /// model, never the name the owner gave the device.
    var deviceLabel: String { get }

    /// Asks Apple for a device token; `didRegister` follows on the coordinator.
    func registerForRemoteNotifications()
}
