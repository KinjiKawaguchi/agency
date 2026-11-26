import Foundation

extension Ja {
    /// The settings of what the app tells the owner (agency#167).
    public enum NotificationSettings {
        public static let appOpenHeading = "アプリを開いているとき"
        public static let appOpenFooter = "画面の上に表示します。"
        public static let appClosedHeading = "アプリを閉じているとき"
        public static let appClosedFooter = "端末に通知します。通知されるのは、依頼、端末の追加、断られた決定です。通知を許可していないときは、端末の設定で許可してください。"

        public static let requestsOnly = "このホームノードは、依頼の通知だけを送ります。"

        public static func title(of kind: EventKind) -> String {
            switch kind {
            case .request: "依頼があなたを待っているとき"
            case .participant: "参加者があなたの返事を待っているとき"
            case .blocked: "仕事が止まったとき"
            case .device: "端末が追加を待っているとき"
            case .decision: "決定に動きがあったとき"
            }
        }

        public static func title(of kind: PushKind) -> String {
            switch kind {
            case .request: title(of: EventKind.request)
            case .device: title(of: EventKind.device)
            case .decision: "決定が断られたとき"
            }
        }
    }
}
