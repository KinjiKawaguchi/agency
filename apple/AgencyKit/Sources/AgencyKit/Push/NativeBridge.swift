import Foundation

/// The script that tells the web client what the app knows, so the page can register this device's
/// push token with the node.
///
/// The page sees `window.agencyNative = { platform: "ios", push: null | { token, environment, label } }`,
/// and an `agency:native-push` event with the new `push` as its `detail` whenever it changes while the
/// page is open. The script does nothing on any page whose origin is not the home node's.
public enum NativeBridge {
    public static let eventName = "agency:native-push"

    /// - Parameter announcesChange: also dispatch the event. It is for a page that is already open;
    ///   a page that is just loading reads the state when it starts.
    public static func script(homeNode: HomeNodeURL, push: PushRegistration?, announcesChange: Bool) -> String {
        let origin = jsonLiteral(homeNode.origin)
        let state = push.map(jsonLiteral) ?? "null"
        let announcement = announcesChange
            ? "window.dispatchEvent(new CustomEvent(\(jsonLiteral(eventName)), { detail: push }));"
            : ""
        return """
        (function () {
          if (window.location.origin !== \(origin)) { return; }
          var push = \(state);
          window.agencyNative = { platform: "ios", push: push };
          \(announcement)
        })();
        """
    }

    private static func jsonLiteral(_ value: some Encodable) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        guard let data = try? encoder.encode(value), let text = String(data: data, encoding: .utf8) else {
            return "null"
        }
        return text
    }
}
