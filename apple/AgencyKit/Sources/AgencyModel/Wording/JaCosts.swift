import Foundation

extension Ja {
    /// The costs screen, from the web client's Japanese (`costs` in `ja/costs.ts`).
    public enum Costs {
        public static let title = "コスト"
        public static let funding = "サブスクリプション"
        public static let noFunding = "資金源はまだありません。"
        public static let period = "期間"
        public static let estimate = "API の料金で換算した目安です。サブスクリプションの請求額ではありません。"
        public static let output = "出力トークン"
        public static let cacheShare = "キャッシュから読んだ入力"
        public static let cacheNote = "高いほど安く済みます"
        public static let byDay = "日ごとのコスト"
        public static let byDayHint = "メンバーごとの積み上げ"
        public static let byParticipant = "メンバーごと"
        public static let other = "その他"
        public static let loading = "トークンの集計を読み込み中…"
        public static let unavailable = "このチームはトークンの使用量を報告していません。"
        public static let costColumn = "API 換算"
        public static let outputColumn = "出力"
        public static let cacheReadColumn = "キャッシュ読み"
        public static let inputWriteColumn = "入力とキャッシュ書き込み"

        public static func tokensTitle(_ team: String) -> String { "\(team) チームのトークン" }
        public static func days(_ count: Int) -> String { "\(count) 日" }
        public static func total(days: Int) -> String { "直近 \(days) 日" }
        public static func average(_ value: String) -> String { "1 日平均 \(value)" }
        public static func inputSide(_ value: String) -> String { "入力側 \(value)" }
        public static func dayTotal(_ date: String, _ total: String) -> String { "\(date): \(total)" }
        public static func couldNotAnswer(_ reason: String) -> String { "チームがいま使用量を返せませんでした: \(reason)" }
    }
}
