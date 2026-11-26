import Foundation

extension Ja {
    /// The outcomes page: the owner's numbers for what the org measures, and how they moved after the work aimed at
    /// them shipped. The words are the web client's, so the two clients say the same. Nothing here says that the work
    /// moved a number: a reading is a correlation, and its line says so.
    public enum Outcomes {
        /// The page's name; the tab it sits in is `Ja.Tabs.results`.
        public static let title = "成果の指標"
        /// Said once on the page: what the numbers are for.
        public static let note = "数値は、指標の動きを読むためのものです。会計用ではありません。"
        public static let readingsHeading = "読み取り"
        public static let noAims = "この組織のチームには、まだ狙いがありません。"
        public static let noKpis = "この組織には、まだ指標がありません。"
        public static let noNumber = "この期間に数値がありません。"
        public static let gap = "—"

        public static func lastEntered(_ day: String) -> String { "最後に入れた数値: \(day)" }
        public static func lastNumber(_ value: String, day: String) -> String { "最後の数値: \(value)、\(day)" }

        // MARK: Movement

        public static let rose = "上がった"
        public static let fell = "下がった"
        public static let unchanged = "変わらず"
        public static let better = "良い方"
        public static let worse = "悪い方"

        /// "上がった +4ポイント（+10%）良い方": the direction, the change, the change as a share of where it began (left out when
        /// that was 0), and, when the KPI says which way is better, whether this is it. No change is only "変わらず".
        public static func movement(_ direction: String, change: String?, ratio: String?, tone: String?) -> String {
            guard let change else { return direction }
            return direction + " " + change + (ratio.map { "（\($0)）" } ?? "") + (tone ?? "")
        }

        // MARK: Readings, one line for each state

        /// What a ready reading says besides its numbers: that the number moved after the work, not that the work moved it.
        public static let correlation = "完了のあとで数値が動きました。この仕事が動かしたとは言えません。"
        public static let notShipped = "まだ完了していないため、比べるものがありません。"
        public static let shipDateUnknown = "完了した日が分からないため、比べられません。"
        public static func noBaseline(day: String) -> String { "完了の前の 90 日に数値がありません。\(day) 以前の日付の数値を入れてください。" }
        public static func tooEarly(readableFrom day: String) -> String { "まだ早すぎます。変化が出るまで 14 日かかります。\(day) から読めます。" }
        public static let noFollowUp = "完了のあとの 90 日に数値がなく、比べられません。これから入れても読めません。"
        public static func tooFew(points: Int, needs: Int) -> String { "完了の前後の数値が足りません: \(points) / \(needs)" }
        /// A state a newer node has: nothing is claimed of it.
        public static let unreadable = "この読み取りは、このアプリではまだ読めません。"
        public static func overlap(_ epics: [String]) -> String { "同じ期間に完了した次の仕事とは区別できません: \(epics.joined(separator: "、"))" }

        /// The numbers of a ready reading: before and after the day the work shipped, each with its day.
        public static func ready(before: String, beforeDay: String, after: String, afterDay: String) -> String {
            "完了の前 \(before)（\(beforeDay)）→ 完了のあと \(after)（\(afterDay)）"
        }

        /// A change of a percent KPI is in points of that percent.
        public static let points = "ポイント"

        /// The names of the two periods the page offers.
        public static func period(_ days: Int) -> String { "\(days) 日" }
    }
}
