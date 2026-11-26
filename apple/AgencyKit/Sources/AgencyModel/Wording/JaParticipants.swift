import Foundation

extension Ja.Evaluation {
    /// What each participant of a team did over the period (agency#364). The words are the web's where it has them.
    public enum Participants {
        public static let title = "参加者"
        public static let busy = "作業"
        public static let waiting = "判断待ち"
        public static let idle = "待機"
        /// Waiting is counted only where the team's runtime sees the wait.
        public static let waitingNote = "判断待ちは、人の判断を待っていた時間の下限です。ランタイムが見える範囲だけを数えます。"
        public static func merged(_ count: Int) -> String { "\(count) 件をマージ" }
        public static func costPerPullRequest(_ cost: String) -> String { "1 件あたり \(cost)" }
        public static let noPullRequest = "マージなし"
        public static func unattributed(_ count: Int) -> String { "参加者が分からないマージ: \(count) 件" }
        public static let none = "この期間に動いた参加者はいません。"
        public static func tokens(output: String, cacheRead: String) -> String { "出力 \(output)・キャッシュ読み \(cacheRead)" }
    }
}
