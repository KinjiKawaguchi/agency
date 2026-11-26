import Foundation

extension Ja {
    /// What the in-app banner says when something new needs the viewer (agency#167).
    public enum News {
        public static func request(team: String, title: String) -> String { "\(team): 「\(title)」があなたを待っています" }
        public static func requests(_ count: Int) -> String { "\(count) 件の依頼があなたを待っています" }

        public static func participant(label: String) -> String { "\(label) があなたの返事を待っています" }
        public static func participants(_ count: Int) -> String { "参加者 \(count) 人があなたの返事を待っています" }

        public static func blocked(team: String, title: String, reason: String) -> String { "\(team): 「\(title)」が止まりました (\(reason))" }
        public static func blockedMany(_ count: Int) -> String { "\(count) 件の仕事が止まりました" }

        public static func device(label: String) -> String { "端末「\(label)」が追加を待っています" }
        public static func devices(_ count: Int) -> String { "\(count) 台の端末が追加を待っています" }

        /// One decision's news. `title` is the decision's title when the log has one; `reason` is the node's
        /// reason for refusing it.
        public static func decision(title: String?, state: String, reason: String?) -> String {
            let named = title.map { "「\($0)」の決定" } ?? "決定"
            switch state {
            case "applied": return "\(named)が反映されました"
            case "delivered": return "\(named)が届きました"
            case "queued": return "\(named)は送信待ちです"
            case "refused": return "\(named)は断られました" + (reason.map { ": \($0)" } ?? "")
            default: return "\(named)に動きがありました"
            }
        }

        public static func decisions(_ count: Int) -> String { "\(count) 件の決定に動きがありました" }
        public static let unknownTitle = "(題名なし)"
        public static let close = "閉じる"
    }
}
