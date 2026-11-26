import AgencyAPI
import Foundation

extension Ja.Outcomes {
    /// Entering a number of a KPI: the web's words (`outcomes.enter`).
    public enum Enter {
        public static let open = "数値を入れる"
        public static let close = "閉じる"
        public static let day = "日付"
        public static let value = "数値"
        public static let note = "メモ（任意）"
        public static let save = "保存"
        public static let saving = "保存しています…"
        public static func saved(_ value: String, day: String) -> String { "\(day) の \(value) を保存しました。" }

        /// In which unit to write it.
        public static func hint(currency: String) -> String { "単位は \(currency) です。" }
        public static let hintPercent = "単位はパーセントです。41.5 は 41.5% です。"
        public static let hintCount = "単位は件数などの数です。"
        public static let hintRatio = "単位のない数です。1.5 は 1.5 倍です。"
        public static let hintSeconds = "単位は秒です。"
        public static let hintUnknown = "単位のない数です。"
        /// In which unit to write a number of a KPI.
        public static func hint(for unit: Kpi.Unit) -> String {
            switch unit {
            case .money(let currency): hint(currency: currency)
            case .percent: hintPercent
            case .count: hintCount
            case .ratio: hintRatio
            case .seconds: hintSeconds
            case .unknown: hintUnknown
            }
        }

        public static let again = "同じ日に 2 つ目を入れると、どの読み取りでも 2 つ目が使われます。1 つ目は履歴に残ります。"

        public static let problemDate = "日付は実在する日で、今日より 1 日先までの、10 年以内の日付にしてください。"
        public static let problemValue = "数値は数字で書いてください（マイナス記号と小数点は使えます）。大きさは 1,000,000,000,000,000 までです。"
        public static let problemNote = "メモは 200 文字までで、制御文字を含めないでください。"

        public static let full = "この指標には、ノードが保てる上限の 5,000 件の数値がすでにあります。"
        public static let forbidden = "数値を入れられるのはオーナーだけです。"
        public static func refused(_ message: String) -> String { "ノードが受け付けませんでした: \(message)" }
    }

    /// The history of the numbers entered for a KPI: the web's words (`outcomes.history`).
    public enum History {
        public static let show = "履歴を見る"
        public static let hide = "履歴を隠す"
        public static let title = "履歴"
        public static let empty = "まだ数値が入っていません。"
        public static let manual = "手で入れた数値"
        public static func connector(_ name: String) -> String { "\(name) から" }
        public static func enteredOn(_ when: String) -> String { "\(when) に入力" }
        public static let replaced = "あとの入力で置き換えられました"
    }
}
