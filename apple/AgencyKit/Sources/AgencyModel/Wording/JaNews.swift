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

        public static func decision(title: String, state: String) -> String {
            switch state {
            case "applied": "「\(title)」の決定が反映されました"
            case "delivered": "「\(title)」の決定が届きました"
            case "queued": "「\(title)」の決定は送信待ちです"
            case "refused": "「\(title)」の決定は断られました"
            default: "「\(title)」の決定に動きがありました"
            }
        }

        public static func decisions(_ count: Int) -> String { "\(count) 件の決定に動きがありました" }
        public static let unknownTitle = "(題名なし)"
        public static let close = "閉じる"
    }
}
