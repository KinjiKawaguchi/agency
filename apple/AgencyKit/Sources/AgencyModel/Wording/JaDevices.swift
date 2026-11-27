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
        /// Removing one of the person's devices, in the web's words (`devices.remove` in `ja.ts`, agency#442) with this phone's own sentence for the
        /// device in use: its key cannot be added again, and the way back is a new key made in the settings.
        public enum Remove {
            public static let button = "削除"
            public static func buttonFor(_ label: String) -> String { "\(label)を削除" }
            public static func title(_ label: String) -> String { "\(label)を削除しますか?" }
            public static func body(_ label: String, code: String?) -> String {
                code.map { "\(label)(照合コード \($0))は、二度と署名できなくなります。" } ?? "\(label)は、二度と署名できなくなります。"
            }
            public static func fromOrgs(_ orgs: String) -> String { "\(orgs) から削除されます。" }
            public static let thisDevice = "いま使っている端末です。削除すると、この端末はこの鍵では署名できなくなり、この鍵をもう一度追加することもできません。そのあと端末は新しい鍵を作り、それは新しい端末として追加する必要があります。"
            /// Said after this device's own removal took effect and it made a new key: the removed key cannot sign, and the new one is not added yet.
            public static let thisDeviceRemoved = "この端末を削除しました。この端末は新しい鍵を作りました。まだ追加されていません。また決めるには、この画面で追加を依頼してください。"
            /// Said instead of the confirmation when this device is the one to remove and decisions signed with its key wait to be sent.
            public static let keptDecisions = "まだ送れていない決定がこの端末に残っています。この端末を削除すると、この鍵で署名したそれらの決定はすべて無効になります。送り終えてから削除してください。"
            /// Said when the list the node gave just before the removal is not the one the person confirmed.
            public static let listChanged = "端末の一覧が、確認したときと変わっていました。一覧を読み直したので、もう一度確認してください。"
            public static func keyNotMade(_ reason: String) -> String { "端末は削除しましたが、この端末は新しい鍵を作れませんでした: \(reason)" }
            public static let confirm = "この端末を削除する"
            public static let cancel = "キャンセル"
            public static let removing = "削除しています…"
            public static func removed(_ label: String) -> String { "\(label)を削除しました。二度と署名できません。" }
            public static func stillIn(_ orgs: String) -> String { "\(orgs) にはまだ載っています。この端末はそこに追加されていないため、削除に署名できません。" }
            public static func last(_ label: String, orgs: String) -> String {
                "\(label)は、\(orgs)でのただ 1 つの端末です。削除すると、そこでは誰も署名できなくなるため、ここでは削除できません。先に別の端末を追加してください。このノードを動かしているマシンで agency device-add を実行しても、追加できます。"
            }
            public static func failed(_ reason: String) -> String { "端末を削除できませんでした: \(reason)" }
            /// The removal is one the app does not make (it checks first, so this is for a list that changed meanwhile).
            public static let cannotHere = "この端末は、ここでは削除できません。一覧を読み直してください。"
            /// Said when the root's node holds the removal for its owner to sign: the same sentence as for an approval, with its own verb.
            public static func pendingRoot(command: String) -> String {
                "ここでは削除を依頼しました。ルート鍵のノードが署名すると有効になります。そのノードで \(command) を実行してください。"
            }
            public static func faceIDReason(_ label: String) -> String { "端末を削除する: \(label)" }
        }

        /// An outcome the app does not know, with no message of the node's: never a success, so it says what is known and where the result is.
        public static let unknownOutcome = "ノードが、このアプリの知らない答えを返しました。結果は端末の一覧で確かめてください。"

        /// What is said for an outcome the app does not know: the node's own message when it has one, and else that it is not known.
        public static func unknownOutcome(_ message: String?) -> String {
            guard let message, !message.isEmpty else { return unknownOutcome }
            return message
        }

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
            "この端末へのプッシュ通知を、APNs は最後に受け付けませんでした（\(when)）: \(reason)。ノードは最大 1 日のあいだ再送します。失敗が続くときは、端末の通知の設定と、ノードの APNs の設定を確かめてください。"
        }

        public static func lastTaken(_ when: String) -> String { "最後に受け付けた通知は \(when) のものです。" }
        public static let neverTaken = "まだ 1 件も受け付けていません。"
    }
}

