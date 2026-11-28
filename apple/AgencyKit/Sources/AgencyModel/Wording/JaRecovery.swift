import Foundation

extension Ja {
    /// A started recovery of an org, on the desk (the same words as the web's, and the glossary's: ルートキーの回復, リカバリーキー). It says what is happening and how to stop it,
    /// and never offers to hide it: the owner must be able to cancel it while it waits.
    public enum Recovery {
        /// For the owner of the root key (`OrgRef.isRoot`), who can cancel it.
        public static func title(org: String) -> String { "\(org) のルートキーの回復が始まりました。" }
        /// For anyone else the org's root change concerns: it is said, and who can cancel it.
        public static func titleForMember(org: String) -> String { "\(org) のルートキーの回復が進んでいます。" }
        public static let ownerCanCancel = "取り消せるのは、その組織のルートキーのノードの管理者です。"
        public static func takesEffect(at time: String) -> String { "\(time) に有効になります。" }
        public static let notYours = "心当たりがないときは、組織のルートキーのノードで取り消してください:"
        public static func cancelCommand(orgID: String) -> String { "agency root-cancel-recovery --org \(orgID)" }
        /// When the org's id is not one the app will put in a command to paste (the node's rule for an id): where to run it, in
        /// words, as the web says it.
        public static let cancelInWords = "心当たりがないときは、組織のルートキーのノードで、この組織を指定して agency root-cancel-recovery を実行し、取り消してください。"
        public static func newRootKey(_ fingerprint: String) -> String { "新しいルートキー: \(fingerprint)" }
        public static let replacesRecoveryKey = "リカバリーキーも取り替えます。"
    }
}
