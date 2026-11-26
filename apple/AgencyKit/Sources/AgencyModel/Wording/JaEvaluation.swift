import AgencyAPI
import Foundation

extension Ja {
    /// The evaluation screen, from the web client's Japanese (`evaluation` in `ja.ts`).
    public enum Evaluation {
        public static let title = "評価"
        public static let lead = "組織が新機能をどれだけ速く出しているか、その費用はいくらか。数字はノードが返すので、エージェントも同じ数字を引用できます。"
        public static let org = "組織"
        public static let period = "期間"
        public static let loading = "数字を読み込んでいます…"
        public static let noOrgs = "まだどの組織にも入っていません。"
        public static let noTeams = "この組織にはまだチームがありません。"
        public static let unavailable = "このノードはまだ評価の数字を返しません。"
        public static let noData = "この期間はありません"
        public static let noSamples = "データなし"
        public static let unreadable = "ノードの答えを読み取れませんでした。"

        public static func days(_ count: Int) -> String { "\(count) 日" }
        public static func team(_ label: String) -> String { "\(label)チーム" }
        public static func couldNotAnswer(_ reason: String) -> String { "チームが今は数字を返せませんでした: \(reason)" }
        public static func teamUnavailable(_ reason: String) -> String { "このチームは数字を返せませんでした: \(reason)" }

        public static func kind(_ kind: MergedCounts.Kind) -> String {
            switch kind {
            case .feat: "新機能"
            case .fix: "修正"
            case .refactor: "リファクタリング"
            case .docs: "ドキュメント"
            case .test: "テスト"
            case .chore: "雑務"
            case .other: "その他"
            }
        }

        public enum Tile {
            public static let features = "マージした新機能"
            public static let merged = "マージしたプルリクエスト"
            public static let prLead = "リードタイム (プルリクエスト)"
            public static let issueLead = "リードタイム (issue)"
            public static let red = "main が赤かった時間"
            public static let reverts = "リバート"
            public static let cost = "マージ 1 件あたりの費用"
            public static let costNote = "API 価格での概算"

            public static func perDay(_ value: String) -> String { "1 日あたり \(value)" }
            public static func leadNote(_ p90: String) -> String { "中央値。10 件中 9 件は \(p90) 以内。" }
            public static func redNote(_ longest: String) -> String { "最長 \(longest)" }
            public static func revertsNote(_ merged: Int) -> String { "マージ \(merged) 件のうち" }
        }

        public enum Chart {
            public static let merged = "1 日にマージしたプルリクエスト"
            public static let mergedHint = "タイトルの種類別"
            public static let lead = "リードタイム"
            public static let leadHint = "直近 7 日の中央値"
            public static let prLead = "プルリクエストを開いてからマージまで"
            public static let issueLead = "issue に着手してからマージまで"
            public static let red = "main の CI が赤かった時間 (1 日ごと)"
            public static let reverts = "1 日あたりのリバート"
            public static let cost = "マージ 1 件あたりの費用"
            public static let costHint = "直近 7 日、API 価格"

            public static func dayMerged(_ date: String, _ count: String) -> String { "\(date): \(count) 件マージ" }
        }
    }
}
