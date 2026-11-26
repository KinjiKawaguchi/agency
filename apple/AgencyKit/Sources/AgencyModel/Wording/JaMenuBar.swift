import Foundation

extension Ja {
    /// The Mac's menu-bar item.
    public enum MenuBar {
        public static let open = "Agency を開く"
        public static let quit = "Agency を終了"
        public static let notRead = "まだ読み込んでいません"
        public static let nothingWaiting = "あなたを待っている依頼はありません"
        public static func waiting(_ count: Int) -> String { "\(count) 件があなたを待っています" }

        /// The text of the item's label beside its symbol: the number when something waits, nothing otherwise.
        public static func label(_ count: Int?) -> String? {
            guard let count, count > 0 else { return nil }
            return "\(count)"
        }

        /// The menu's first line.
        public static func status(_ count: Int?) -> String {
            guard let count else { return notRead }
            return count == 0 ? nothingWaiting : waiting(count)
        }
    }
}
