import AgencyKit
import UserNotifications

/// Sets the icon's number to the sum over the home nodes when an alert arrives, and words a recovery's and a held root change's
/// alert in the viewer's language.
///
/// A push's badge is its own node's count (`aps.badge`). The extension records it for the node the push names (`agency.node`)
/// and shows the sum of what each node of this device last said (`BadgeLedger`, in the app group's container, which the app
/// keeps). It reads no network and keeps no title. When anything is missing it delivers the notification as it came; the
/// decision is `BadgeFromPush`, which is tested in the package.
final class NotificationService: UNNotificationServiceExtension {
    private var contentHandler: ((UNNotificationContent) -> Void)?
    private var delivered: UNMutableNotificationContent?

    override func didReceive(_ request: UNNotificationRequest, withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void) {
        self.contentHandler = contentHandler
        let content = request.content.mutableCopy() as? UNMutableNotificationContent
        delivered = content
        guard let content else {
            contentHandler(request.content)
            return
        }
        // A recovery that was started and a change held for the root key's signature are worded here, in the viewer's language and
        // zone, from the facts the node puts in the payload; the node's own English is for a client that does not know the kind.
        if let body = PushNotice.body(
            userInfo: request.content.userInfo, language: Locale.preferredLanguages.first ?? "en", timeZone: TimeZone.current
        ) {
            content.title = PushNotice.title
            content.body = body
        }
        if let file = Self.ledgerFile,
           let badge = BadgeFromPush.badge(userInfo: request.content.userInfo, pushBadge: request.content.badge?.intValue, file: file) {
            content.badge = NSNumber(value: badge)
        }
        contentHandler(content)
    }

    /// The time ran out: what there is goes out, as it came when nothing was changed.
    override func serviceExtensionTimeWillExpire() {
        if let contentHandler, let delivered { contentHandler(delivered) }
    }

    private static var ledgerFile: BadgeLedgerFile? {
        (Bundle.main.object(forInfoDictionaryKey: "AgencyAppGroup") as? String).flatMap(BadgeLedgerFile.appGroup)
    }
}
