import AgencyAPI
import AgencyProtocol
import Foundation

/// The words of the app, in Japanese, taken from the web client (`web/src/i18n/ja/`) where it has them.
/// They are plain Swift so they are tested on every platform, and the screens only place them.
/// Nothing here describes keys or shortcuts: the phone has no keyboard to explain.
public enum Ja {
    public enum Desk {
        public static let title = "あなた宛てのリクエスト"
        public static let pileLabel = "届いている書類"
        public static let empty = "待っているものはありません。チームからの新しいリクエストは、ここに出ます。"
        public static let nothingToDecide = "今、あなた宛てのリクエストはありません。"
        public static let recentHead = "最近決めたもの"
        public static let loading = "読み込み中…"
        public static let reload = "再読み込み"
        public static let signInEnded = "サインインの有効期限が切れました。もう一度サインインしてください。"
        public static let signInAgain = "サインインし直す"
        public static let signInUnavailable = "このノードでは、アプリからのサインインを使えません。ノードの設定を確認してください。"
        public static let settings = "設定"
        public static let changeNode = "このホームノードから出る"
        public static let homeNode = "ホームノード"
        public static let close = "閉じる"

        public static func group(_ group: DeskGroup) -> String {
            switch group {
            case .proposal: "決裁する稟議"
            case .question: "あなたへの質問"
            case .pendingDecision: "ボードで相談待ちのもの"
            case .unrecognized: "このアプリが知らない種類のリクエスト"
            case .others: "ほかの人の返事待ち"
            }
        }

        public static func kind(_ kind: Request.Kind) -> String {
            switch kind {
            case .proposal: "稟議"
            case .question: "質問"
            case .pendingDecision: "相談"
            case .unknown: "リクエスト"
            }
        }

        public static func itemsOnDesk(_ count: Int) -> String { "机の上に \(count) 件あります。" }
        public static func someoneWaiting(_ count: Int) -> String { "メンバー \(count) 人があなたの返事を待っています。" }
        public static func peopleWorking(working: Int, total: Int) -> String { "メンバー \(total) 人のうち \(working) 人が作業中です。" }
        public static func blocking(_ count: Int) -> String { "メンバー \(count) 人が待機" }
        public static func drafter(_ name: String) -> String { "起案: \(name)" }
        /// A request addressed to a seat or an external party that the viewer answers for: whom it is for (the web's words).
        public static func answerFor(_ label: String) -> String { "\(label) の代理回答" }
        public static func unreadableTeam(_ team: String, reason: String) -> String { "\(team) を読み込めませんでした: \(reason)" }
        public static func loadFailed(_ reason: String) -> String { "ノードに接続できません: \(reason)" }

        public static func staleCopy(when: String, reason: String) -> String {
            "このチームは \(when) に読んだ写しを表示しています。チームのノードに接続できません: \(reason)"
        }

        public static func neverRead(reason: String) -> String { "このチームのノードはまだ読めていません: \(reason)" }
    }

    public enum Paper {
        public static let options = "選択肢"
        public static let recommended = "起案者のおすすめ"
        public static let note = "補足 (任意)"
        public static let notePlaceholderWithOptions = "条件や補足があれば書いてください (任意)"
        public static let notePlaceholderOpen = "答えを書いてください"
        public static let noteLabelOpen = "答え"
        public static let sendComment = "コメントだけ送る"
        /// What the comment button does. 決裁 is a proposal's word; a question or a decision is "decided".
        public static func commentExplain(_ kind: Request.Kind) -> String {
            let nothing = kind == .proposal ? "決裁はしません。" : "決めません。"
            return nothing + "この文章だけがチームに届きます。選択肢を選んでいても、選択は送られません。"
        }
        public static let sending = "送信中…"
        public static let earlierMessages = "これまでのやりとり"
        public static let decisionLog = "決定の記録"
        public static let needChoiceOrComment = "選択肢を選ぶか、コメントを書いてください。"
        public static let needAnswer = "答えを書いてください。"
        public static let needDecision = "判断を書いてください。"
        public static let queued = "保存しました。チームのノードが休止中なので、起動したときに届きます。"
        public static let cannotAnswerHere = "このリクエストには、このアプリではまだ答えられません。アプリを新しくするか、ほかの端末で答えてください。"

        public static func type(_ kind: Request.Kind) -> String {
            switch kind {
            case .proposal: "稟議"
            case .question: "質問"
            case .pendingDecision: "相談"
            case .unknown: "リクエスト"
            }
        }

        public static func verb(_ kind: Request.Kind) -> String {
            switch kind {
            case .proposal: "決裁する"
            case .question: "答える"
            case .pendingDecision: "判断を送る"
            case .unknown: "返事を送る"
            }
        }

        /// What Face ID says it is for: the action, then the request's title.
        public static func faceIDReason(_ decision: PreparedDecision) -> String {
            let action = decision.isComment ? "コメントを送る" : verb(decision.kind)
            return "\(action): \(NodeLabel.shown(decision.title))"
        }

        public static func sendWith(_ verb: String, choice: String) -> String { "「\(choice)」で\(verb)" }
        public static func stoppedBy(_ names: String) -> String { "\(names) は、これが決まるまで先に進めません" }
        public static func waitingOn(_ name: String) -> String { "\(name) さんへの相談です。ここで読むことはできますが、答えることはできません。" }
        public static func readDocument(characters: Int) -> String { "本文を読む (約 \(characters) 文字)" }
        public static func sendFailed(_ reason: String) -> String { "送れませんでした: \(reason)" }
        public static func team(org: String, team: String) -> String { "\(org) / \(team)" }
        public static func thread(name: String, choice: String?) -> String { choice.map { "\(name) は「\($0)」を選びました" } ?? name }
    }

    public enum Device {
        public static let heading = "この端末を追加してください"
        public static let why = "決めたり、コメントしたりするには、この端末の鍵をあなたの組織に追加する必要があります。読むだけなら、今のままでも使えます。"
        public static let add = "この端末を追加する"
        public static let adding = "申し込み中…"
        public static let ready = "この端末は追加されています。"
        public static let label = "iPhone"
        public static let macLabel = "Mac"
        public static let reset = "この端末の鍵を作り直す"
        public static let resetConfirm = "作り直す"

        public static func notAdded(org: String) -> String { "この端末は \(org) に追加されていません。先に端末を追加してください。" }
        public static func failed(_ reason: String) -> String { "端末の状態を調べられませんでした: \(reason)" }
    }

    public enum Recent {
        /// A decision that answered for a seat or an external party (the web's words).
        public static func answeredFor(_ label: String) -> String { "\(label) の代理で答えました。" }
        public static func state(_ state: DecisionState) -> String {
            switch state {
            case .applied: "反映済み"
            case .delivered: "届きました"
            case .queued: "保存済み・未配達"
            case .refused: "断られました"
            case .unknown: "状態を読み取れません"
            }
        }

        public static func chose(_ choice: String) -> String { "「\(choice)」を選びました。" }
        public static let commented = "コメントしました。"

        public static func queuedDetail(attempts: Int, retry: String?) -> String {
            let tried = attempts == 0 ? "まだ届いていません。" : "\(attempts) 回試しました。"
            let next = retry.map { "次の再送は \($0) です。" } ?? "まもなく再送します。"
            return tried + next
        }

        public static func lastError(_ reason: String) -> String { "直近のエラー: \(reason)" }
        public static func refusedDetail(_ reason: String) -> String { "チームのノードが受け付けませんでした: \(reason)" }
        public static func unavailable(_ reason: String) -> String { "最近の決定を読み込めませんでした: \(reason)" }
    }
}

/// How long ago something happened, as the web client says it.
public enum Ago {
    public static let justNow = "たった今"

    public static func text(from date: Date, now: Date) -> String {
        let delta = date.timeIntervalSince(now)
        let size = abs(delta)
        let isPast = delta <= 0
        if size < 60 { return justNow }
        let (count, unit) = size < 3600 ? (Int((size / 60).rounded()), "分") : size < 2 * 86_400 ? (Int((size / 3600).rounded()), "時間") : (Int((size / 86_400).rounded()), "日")
        return "\(count) \(unit)\(isPast ? "前" : "後")"
    }
}
