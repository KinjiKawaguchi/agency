import Foundation

extension Ja {
    /// The words of the devices screen, the web client's where it has them.
    public enum Devices {
        public static let title = "端末"
        public static let intro = "決定を下すブラウザ、スマートフォン、コマンドラインは、それぞれ自分の鍵を持ちます。鍵は端末の中に残り、外へ取り出せません。決定にはこの鍵で署名します。"
        public static let thisDevice = "この端末"
        /// The heading of the card that says whether APNs takes this phone's notices.
        public static let pushTitle = "プッシュ通知"
        public static let codeLabel = "この端末の照合コード"
        public static let loadingList = "ノードに端末の一覧を問い合わせています…"
        public static let added = "この端末は追加済みです。決定には、この端末の鍵で署名します。"
        public static let yourDevices = "あなたの端末"
        public static let thisOne = "この端末"
        public static let requests = "追加を待っている端末"
        public static let noRequests = "追加を待っている端末はありません。"
        public static let compare = "このコードを新しい端末のコードと見比べてください。1 文字でも違えば、追加しないでください。"
        public static let confirm = "コードが一致したので承認する"
        public static let confirming = "追加しています…"
        public static let nowhereToAdd = "追加できる組織がありません。この端末が追加されている組織が必要です。"
        public static let accountUnknown = "アカウントを読めませんでした。"
        public static let codeMismatch = "ノードが示したコードが、この端末の鍵から計算したコードと違います。追加しないでください。"
        public static let malformedRequest = "この依頼の鍵が正しくありません。追加しないでください。"

        public static func addedAs(_ id: String) -> String { "端末 ID は \(id) です。" }
        public static func addedOn(_ when: String) -> String { "\(when)に追加" }
        public static func requestedAt(_ when: String) -> String { "\(when)に依頼" }
        public static func codeOf(_ label: String) -> String { "\(label) の照合コード" }
        public static func inOrgs(_ orgs: String) -> String { "組織: \(orgs)" }
        public static func confirmed(_ label: String) -> String { "\(label) を追加しました。" }
        public static func confirmFailed(_ reason: String) -> String { "追加できませんでした: \(reason)" }
        public static func listFailed(_ reason: String) -> String { "端末の一覧を読めませんでした: \(reason)" }
        public static func faceIDReason(_ label: String) -> String { "端末を追加する: \(label)" }
        public static func queued(_ message: String?) -> String {
            "保存しました。組織のノードが休止中なので、起動したときに届きます。" + (message.map { " (\($0))" } ?? "")
        }
    }
}

extension Ja {
    /// Whether APNs takes the notices the node sends this phone, in the words of the web's card for a browser (`notices.os.push`), with APNs
    /// in the place of the browser's push service. The reason is the node's own text (the status and reason code APNs answered), never
    /// APNs' own words.
    public enum PushDelivery {
        public static func isFailing(_ when: String, _ reason: String) -> String {
            "この端末へのプッシュ通知を、APNs は最後に受け付けませんでした（\(when)）: \(reason)。ノードは最大 1 日のあいだ再送します。失敗が続くときは、端末の通知の設定を確かめてください。"
        }

        public static func lastTaken(_ when: String) -> String { "最後に受け付けた通知は \(when) のものです。" }
        public static let neverTaken = "まだ 1 件も受け付けていません。"
        /// The node lists this phone's token and has sent it no notice yet, or has no word of it.
        public static let noneYet = "APNs は、この端末にまだ通知を受け付けていません。"
    }
}

