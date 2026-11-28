import Foundation

extension Ja {
    /// The org chart, from the web client's Japanese (`org` in `ja/org.ts`).
    public enum OrgChart {
        public static let title = "組織図"
        public static let allTitle = "あなたのすべての組織"
        public static let allOrgs = "すべての組織"
        public static let org = "組織"
        public static let orgUnavailable = "読み込めません"
        public static let whole = "組織全体に戻る"
        public static let noHead = "責任者はいません"
        public static let you = "あなた"
        public static let answerer = "回答のみ"
        public static let oldData = "古いデータ"
        public static let emptyDepartment = "この部署にはまだ誰もいません。"
        public static let teamUnavailable = "このチームを読み込めませんでした"
        public static let noOrgs = "まだどの組織にも入っていません。"
        public static let account = "アカウント"
        public static let organization = "組織"
        public static let department = "部署"
        public static let team = "チーム"
        public static let member = "メンバー"

        public static func orgTitle(_ label: String) -> String { "\(label) の組織" }
        public static func ledBy(_ name: String) -> String { "責任者は \(name)" }
        public static func moreAgents(_ count: Int) -> String { "+\(count)" }
    }
}
