import Foundation

extension Ja.Evaluation {
    /// What each participant of a team did over the period (agency#364): the web's words (`evaluation.participants`).
    public enum Participants {
        public static let title = "メンバー"
        public static let lead = "期間中にメンバーがどう過ごしたか、トークンのコスト、そのメンバーの名前でマージされたプルリクエストです。読めるのはチームの既定の回答者だけです。"
        public static let time = "セッションが生きていた時間"
        public static let timeNote = "メンバーのセッションが 1 つ以上生きていた時間を数えます。重なったセッションは 1 回として数えます。相談待ちは、人の判断を待っていた時間の下限です。ランタイムが見える範囲だけを数えます。"
        public static let working = "作業"
        public static let waiting = "相談待ち（下限）"
        public static let idle = "待機"
        public static let noSession = "この期間に生きていたセッションはありません。"
        public static let cost = "コスト"
        public static func tokens(_ count: String) -> String { "\(count) トークン" }
        public static let merged = "マージ"
        public static func mergedCount(_ count: Int) -> String { "マージしたプルリクエスト \(count) 件" }
        public static let perPullRequest = "マージしたプルリクエスト 1 件あたりのコスト"
        public static let noMerged = "マージなし"
        public static func unattributed(_ count: Int) -> String {
            "メンバーが分からないマージ: \(count) 件。runtime が作者の名前を返しませんでした。メンバーごとのマージ数とこの件数を足すと、チームのマージ数になります。"
        }

        /// What a screen reader says of a participant's time, for the bar that shows it.
        public static func timeSpoken(working: String, waiting: String, idle: String) -> String {
            "作業 \(working)、相談待ち \(waiting) 以上、待機 \(idle)"
        }
    }
}
