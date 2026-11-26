import Foundation

extension Ja {
    /// What a region says when its data keeps not coming (agency#170): why, when it is asked again, and a button.
    public enum Loading {
        /// How long after a failure the screen asks again by itself.
        public static let pollSeconds = 30

        public static let tryAgain = "もう一度試す"
        public static let loading = "読み込んでいます…"

        public static func couldNotLoad(_ what: String, why: String) -> String { "\(what)を読み込めませんでした: \(why)" }
        public static func nextTry(afterSeconds seconds: Int = pollSeconds) -> String { "\(seconds) 秒後にもう一度試します。" }
    }
}
