import AgencyAPI

/// Whether a push registration names this phone as one of the account's devices, so that the node drops the registration when the
/// device is removed. The choice is the app's, from what the node says of itself and from the list of devices it gave.
public enum PushDeviceChoice {
    /// The id to send as the registration's `device`, or `nil` to send none.
    ///
    /// Nothing is named unless the node lists `pushDevice`: any other node answers 400 for the field. And only a device the node lists is
    /// named: a phone whose key was not added yet is not one of the account's devices, and the node refuses it. A phone that was named
    /// and is no longer in the list (it was removed) keeps the device it was registered with, so that it is not registered again without it.
    /// - Parameters:
    ///   - about: what the node said of itself; `nil` until it has been read.
    ///   - thisDeviceID: this phone's id in the node's list of devices, when it is in it.
    ///   - registered: the device the last registration named.
    public static func device(about: About?, thisDeviceID: String?, registered: String?) -> String? {
        guard let about, about.has(.pushDevice) else { return nil }
        if let thisDeviceID { return thisDeviceID }
        return registered
    }
}
