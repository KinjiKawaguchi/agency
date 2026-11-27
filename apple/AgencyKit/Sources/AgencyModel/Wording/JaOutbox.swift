import Foundation

extension Ja {
    /// The decisions kept on the device and sent in the background (agency#447): what the tray says. The words are the web's (#446), and the
    /// reason of a failure is shown as the node or the network gave it, never reworded.
    public enum Outbox {
        public static func heading(count: Int) -> String { "送信中の決定（\(count)）" }
        public static let sending = "送信中…"
        /// A send that did not get through and will be tried again by itself.
        public static func waiting(nextInSeconds seconds: Int) -> String {
            if seconds <= 1 { return "まだ送れていません。まもなく再送します。" }
            return "まだ送れていません。次の再送は " + (seconds < 60 ? "1 分以内" : "\((seconds + 30) / 60) 分後") + " です。"
        }
        public static func attempts(_ count: Int) -> String { "\(count) 回試しました。" }
        /// A refusal that sending again will not change: the reason as the node gave it.
        public static func failed(_ reason: String) -> String { "送れませんでした: \(reason)" }
        public static let sendAgain = "もう一度送る"
        public static let discard = "破棄"
        public static let discardExplain = "破棄すると、書いた内容つきで依頼が山に戻ります。"
        public static func more(_ count: Int) -> String { "ほか \(count) 件" }
        /// How many entries the tray lists before it says how many more there are.
        public static let shown = 4
        /// The decision was signed but could not be written to the device, so it was not accepted and nothing was sent.
        public static let notKept = "この端末に決定を保存できなかったため、送っていません。もう一度お試しください。"
        public static let signInEnded = "サインインが切れています。サインインし直すと、続きを送ります。"
    }
}
