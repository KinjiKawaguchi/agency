import Foundation

extension Ja {
    /// The home nodes this device knows, and the way to switch between them (agency#117).
    public enum HomeNodes {
        public static let title = "ホームノード"
        public static let switchHeading = "ホームノードを切り替える"
        public static let current = "いま使っているホームノード"
        public static let others = "ほかのホームノード"
        public static let none = "ほかのホームノードはありません。"
        public static let configured = "このノードの設定にあります"
        public static let add = "ホームノードを追加"
        public static let addressField = "アドレス"
        public static let labelField = "名前（なくてもかまいません）"
        public static let addConfirm = "追加する"
        public static let forget = "この端末から忘れる"
        public static let forgetConfirm = "忘れる"
        public static func forgetExplain(_ host: String) -> String {
            "\(host) のサインインと、この端末の鍵と、書きかけの下書きを、この端末から消します。もう一度使うには、サインインと端末の追加が必要です。"
        }
        public static let switchHint = "切り替えると、このホームノードのサインインと鍵はそのまま残ります。"
        public static let signInTitle = "サインインが必要です"
    }
}
