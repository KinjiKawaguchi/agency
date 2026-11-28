import AgencyAPI
import Foundation

extension Ja {
    /// The evaluation screen, from the web client's Japanese (`evaluation` in `ja/evaluation.ts`).
    public enum Evaluation {
        public static let title = "評価"
        public static let lead = "組織が新機能をどれだけ速く出しているか、そのコストはいくらか。数字はノードが返すので、AI エージェントも同じ数字を引用できます。"
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
            public static let red = "main の CI が失敗していた時間"
            public static let reverts = "リバート"
            public static let cost = "マージ 1 件あたりのコスト"
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
            public static let red = "main の CI が失敗していた時間 (1 日ごと)"
            public static let reverts = "1 日あたりのリバート"
            public static let cost = "マージ 1 件あたりのコスト"
            public static let costHint = "直近 7 日、API 価格"

            public static func dayMerged(_ date: String, _ count: String) -> String { "\(date): \(count) 件マージ" }
        }
    }
}

extension Ja {
    /// Where each team's effort goes, from the web client's Japanese (`evaluation.effort` in `ja/evaluation.ts`).
    public enum Effort {
        public static let title = "労力の行き先"
        public static let lead = "各チームが使ったコストを、仕事の種類別に、組織の意図と並べて見ます。コストは API 価格での概算です。"
        public static let unavailable = "このノードはまだ作業種別ごとの労力を返しません。"
        public static let noPlan = "このチームには組織のプランがまだ示されていないので、比べるものがありません。"
        public static let planUnreadable = "このチームの組織のプランを読み取れなかったので、比べていません。"
        public static let allocationTitle = "作業種別ごとのコストの割合"
        public static let allocationHint = "印は組織が意図する割合"
        public static let onPlan = "プランどおり"
        public static let unclassifiedNote = "作業種別がなく、プルリクエストの種類からも決められない仕事は、未分類のままにします。推測では埋めません。"
        public static let budgetTitle = "週次予算"
        public static let byDay = "1 日ごとのコスト"
        public static let byDayHint = "作業種別別"
        public static let tableShow = "数字を表で見る"
        public static let tableType = "作業種別"
        public static let tableCost = "API 価格"
        public static let tableShare = "割合"
        public static let tableIntended = "意図"
        public static let tableTotal = "合計"

        public static func type(_ type: EffortType) -> String {
            switch type {
            case .newFeature: "新機能"
            case .improvement: "改善"
            case .maintenance: "保守"
            case .debt: "技術的負債"
            case .incident: "障害対応"
            case .unclassified: "未分類"
            }
        }

        public static func couldNotAnswer(_ reason: String) -> String { "チームが今は労力を返せませんでした: \(reason)" }
        public static func actual(_ share: String) -> String { "コストの \(share)" }
        public static func intended(_ share: String) -> String { "意図 \(share)" }
        public static func above(_ points: Int) -> String { "\(points) ポイント上" }
        public static func below(_ points: Int) -> String { "\(points) ポイント下" }
        public static func budgetLine(spent: String, of cap: String, weekStart: String) -> String { "今週(\(weekStart) の月曜から)は \(cap) のうち \(spent)" }
        public static func budgetNotKnown(weekStart: String) -> String { "今週(\(weekStart) の月曜から)の使用額は分かりません" }
        public static func budgetMeter(spent: String, of cap: String, state: String?) -> String {
            "週次予算 \(cap) のうち \(spent) を使用" + (state.map { ": \($0)" } ?? "")
        }
        public static let budgetOK = "予算の範囲内です"
        public static let budgetWarning = "予算に近づいています"
        public static let budgetOverState = "予算を超えています"
        public static let budgetPartial = "このチームの使用量を読めなかったため、今週の使用額は分かりません。"
        public static func budgetOrg(isOver: Bool, spent: String, of cap: String) -> String {
            "組織は週次予算\(isOver ? "を超えています" : "に近づいています"): \(cap) のうち \(spent)。"
        }
        public static let budgetOrgLowerBound = "読めなかった使用量があるため、少なくともこれだけです。"
        public static let budgetOrgNoneRead = "組織の使用量に読めなかったものがあるため、今週の使用額は分かりません。"
        public static func budgetOrgPartialOK(spent: String, of cap: String) -> String { "組織の使用量に読めなかったものがあります: 今週は \(cap) のうち、少なくとも \(spent)。" }
        public static func budgetPlain(_ cap: String) -> String { "週次予算は \(cap) です。このノードは、今週の使用額をまだ返しません。" }
        public static func budgetFailed(_ reason: String) -> String { "週次予算状況を読み込めませんでした: \(reason)" }
    }
}
