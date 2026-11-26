import Foundation

extension Ja.Evaluation {
    /// What each participant of a team did over the period (agency#364): the web's words (`evaluation.participants`).
    public enum Participants {
        public static let title = "参加者"
        public static let lead = "期間中に参加者がどう過ごしたか、トークンのコスト、その参加者の名前でマージされたプルリクエストです。読めるのはチームのオーナーだけです。"
        public static let time = "セッションが生きていた時間"
        public static let timeNote = "参加者のセッションが 1 つ以上生きていた時間を数えます。重なったセッションは 1 回として数えます。判断待ちは、人の判断を待っていた時間の下限です。ランタイムが見える範囲だけを数えます。"
        public static let working = "作業"
        public static let waiting = "判断待ち（下限）"
        public static let idle = "待機"
        public static let noSession = "この期間に生きていたセッションはありません。"
        public static let cost = "コスト"
        public static func tokens(_ count: String) -> String { "\(count) トークン" }
        public static let merged = "マージ"
        public static func mergedCount(_ count: Int) -> String { "マージしたプルリクエスト \(count) 件" }
        public static let perPullRequest = "マージしたプルリクエスト 1 件あたりのコスト"
        public static let noMerged = "マージなし"
        public static func unattributed(_ count: Int) -> String {
            "参加者が分からないマージ: \(count) 件。runtime が作者の名前を返しませんでした。参加者ごとのマージ数とこの件数を足すと、チームのマージ数になります。"
        }

        /// What a screen reader says of a participant's time, for the bar that shows it.
        public static func timeSpoken(working: String, waiting: String, idle: String) -> String {
            "作業 \(working)、判断待ち \(waiting) 以上、待機 \(idle)"
        }
    }
}
