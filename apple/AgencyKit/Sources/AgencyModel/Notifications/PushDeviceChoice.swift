import AgencyAPI

/// Whether a push registration names this phone as one of the account's devices, so that the node drops the registration when the
/// device is removed. The choice is the app's, from what the node says of itself and from the list of devices it gave.
public enum PushDeviceChoice {
    public enum Decision: Equatable, Sendable {
        /// Register without a device: the node does not take one.
        case withoutDevice
        /// Register naming this device.
        case withDevice(String)
        /// Do not register yet: what decides is not known, or this phone is not one of the account's devices.
        case notYet
    }

    /// What to do with the registration.
    ///
    /// A node that lists `pushDevice` ties a registration to a device, and one with no device stays until it is removed by hand: so on such a
    /// node a phone is registered only when it is one of the account's devices (the node refuses any other), and an unlisted phone, such as a
    /// new key after a removal, is not registered at all until it is added, since an unnamed registration would bring a removed phone's
    /// notices back. A node that does not list it is sent none. What the node says of itself is waited for, not guessed.
    /// - Parameters:
    ///   - about: what the node said of itself; `nil` until it has been read.
    ///   - thisDeviceID: this phone's id in the node's list of devices, when it is in it.
    ///   - registered: the device the last registration named. A phone that was named and has left the list keeps it, and is not registered
    ///     again without it.
    public static func decide(about: About?, thisDeviceID: String?, registered: String?) -> Decision {
        guard let about else { return .notYet }
        guard about.has(.pushDevice) else { return .withoutDevice }
        if let thisDeviceID { return .withDevice(thisDeviceID) }
        if let registered { return .withDevice(registered) }
        return .notYet
    }
}
