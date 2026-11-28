import AgencyAPI
import Foundation

extension Ja {
    /// The overview of every org and team, from the web client's Japanese (`overview` in `ja/overview.ts`).
    public enum Overview {
        public static let title = "チームの状況"
        public static let noTeams = "この組織には、まだチームがありません。"
        public static let nothingWaits = "あなたを待っているリクエストはありません。"
        public static let noParticipants = "メンバーの報告がありません。"
        public static let nobodyWorking = "作業中のメンバーはいません。"
        public static let usageNone = "このチームはトークンの使用量を報告していません。"
        public static let usageFailed = "トークンの使用量を読めませんでした。"
        public static let funding = "利用上限"
        public static let thisTeam = "このチーム"
        public static let failedTag = "読み込めません"
        public static let signInNothingToShow = "サインインし直すまで、表示できるものがありません。"

        public static func lead(requests: Int, teams: Int) -> String {
            requests == 0 ? "あなたを待っているリクエストは、どのチームにもありません。" : "\(teams) チームで、\(requests) 件のリクエストがあなたを待っています。"
        }

        public static func waitingForYou(_ count: Int) -> String { "あなたを待っているリクエスト \(count) 件" }
        public static func working(_ working: Int, of total: Int) -> String { "メンバー \(total) 人のうち \(working) 人が作業中" }
        public static func blocked(_ count: Int) -> String { "止まっている項目 \(count) 件" }
        public static func usage(_ amount: String) -> String { "今週 \(amount) (API 換算)" }
        public static func dataFrom(_ relative: String) -> String { "\(relative) のデータ" }
        public static func staleFrom(_ relative: String, reason: String) -> String { "\(relative) のデータです。チームのノードに接続できません: \(reason)" }
        public static func neverRead(reason: String) -> String { "まだ読めていません: \(reason)" }
        public static func loadFailed(_ reason: String) -> String { "このチームを読み込めませんでした: \(reason)" }

        public static func tag(_ attention: Attention) -> String? {
            switch attention {
            case .mine: "あなたの番"
            case .stuck: "止まっている"
            case .idle: "動いていない"
            case .active: nil
            }
        }
    }
}
