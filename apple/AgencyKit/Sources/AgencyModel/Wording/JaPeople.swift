import AgencyAPI
import Foundation

extension Ja {
    /// The participants and work screen, from the web client's Japanese (`people` in `ja/people.ts`).
    public enum People {
        public static let title = "参加者と仕事"
        public static let roleLead = "リード"
        public static let holding = "担当"
        public static let openDecisions = "決めることを開く"
        public static let waitingForAnswer = "あなたの返事を待っています"
        public static let workTitle = "進行中の仕事"
        public static let noWork = "進行中の仕事はありません。"
        public static let blockedTitle = "止まっている仕事"
        public static let unassigned = "担当なし"
        public static let etaNone = "見込みはまだありません"
        public static let epicsTitle = "エピックの見込み"
        public static let noEpics = "エピックはまだありません。"
        public static let noTeams = "まだチームがありません。"
        public static let epicFinished = "完了しました"
        public static let legendDone = "完了"
        public static let legendDoing = "進行中"
        public static let legendBlocked = "停止"
        public static let legendRest = "未着手"

        public static func team(_ label: String) -> String { "\(label) チーム" }
        public static func summary(working: Int, of total: Int) -> String { "参加者 \(total) 人のうち \(working) 人が作業中です。" }
        public static func waitingOnYou(_ count: Int) -> String { "参加者 \(count) 人があなたの返事を待っています。" }
        public static func since(_ when: String) -> String { "\(when) から" }
        public static func moreHeld(_ count: Int) -> String { "ほか \(count) 件" }
        public static func moreWork(_ count: Int) -> String { "あと \(count) 件を表示" }
        public static func blockedSince(_ when: String) -> String { "\(when) から" }
        public static func etaOverBudget(_ basis: String) -> String { "見込みを過ぎています (\(basis))" }
        public static func etaAround(_ when: String) -> String { "\(when) ごろに終わる見込み" }
        public static func etaLatest(_ when: String) -> String { "遅くとも \(when)" }
        public static func epicFinishedAt(_ when: String) -> String { "\(when) に完了しました" }
        public static func epicFinish(_ when: String) -> String { "\(when) ごろに終わる見込み" }
        public static func epicBlockedBy(_ list: String) -> String { "停止: \(list)" }

        public static func epicBar(total: Int, done: Int, doing: Int, blocked: Int) -> String {
            "\(total) 件のうち、完了 \(done)、進行中 \(doing)、停止 \(blocked)"
        }

        /// `nil` for a participant who is idle: nothing is said of them.
        public static func stateTag(_ presence: Presence) -> String? {
            switch presence {
            case .needsYou: "あなたの番"
            case .working: "作業中"
            case .waiting: "待ち"
            case .away: "離席"
            case .idle: nil
            }
        }
    }
}
