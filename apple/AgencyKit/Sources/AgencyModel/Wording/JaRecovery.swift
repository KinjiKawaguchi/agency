import Foundation

extension Ja {
    /// A started recovery of an org, on the desk (the same words as the web's, and the glossary's: ルート鍵の回復, 回復用の鍵). It says what is happening and how to stop it,
    /// and never offers to hide it: the owner must be able to cancel it while it waits.
    public enum Recovery {
        /// For the owner of the root key (`OrgRef.isRoot`), who can cancel it.
        public static func title(org: String) -> String { "\(org) のルート鍵の回復が始まりました。" }
        /// For anyone else the org's root change concerns: it is said, and who can cancel it.
        public static func titleForMember(org: String) -> String { "\(org) のルート鍵の回復が進んでいます。" }
        public static let ownerCanCancel = "取り消せるのは、そのルート鍵の持ち主です。"
        public static func takesEffect(at time: String) -> String { "\(time) に有効になります。" }
        public static let notYours = "心当たりがないときは、ルート鍵のノードで取り消してください:"
        public static func cancelCommand(orgID: String) -> String { "agency root-cancel-recovery --org \(orgID)" }
        public static func newRootKey(_ fingerprint: String) -> String { "新しいルート鍵: \(fingerprint)" }
        public static let replacesRecoveryKey = "回復用の鍵も取り替えます。"
    }
}
