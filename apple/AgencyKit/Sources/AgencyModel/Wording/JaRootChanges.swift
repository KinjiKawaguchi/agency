import Foundation

extension Ja {
    /// The changes others asked the root key to sign, which wait for its owner (the same words as the web's).
    public enum RootChanges {
        /// What the command is run for, and the sentence around it.
        public static let signTogetherBefore = "まとめて署名するには、組織のルートキーのノードで "
        public static let signTogetherAfter = " を実行してください。"

        public static func title(count: Int) -> String { "組織のルートキーの署名を待っている変更が \(count) 件あります。" }
        public static func item(_ what: String, org: String, when: String) -> String { "\(what)（\(org)、\(when)にリクエスト）" }
        /// For a request whose time the node wrote in a form that cannot be read.
        public static func itemWithoutTime(_ what: String, org: String) -> String { "\(what)（\(org)）" }

        public static func addMember(label: String, member: String) -> String { "組織のメンバー \(label)（\(member)）の追加" }
        public static func addDevice(label: String, member: String) -> String { "\(member) の端末「\(label)」の追加" }
        public static func removeDevice(device: String, member: String) -> String { "\(member) の端末 \(device) の削除" }

        /// Said on a device's request once this device approved it and the root's node holds the change.
        public static func approvedHere(command: String) -> String {
            "ここでは承認しました。組織のルートキーのノードが署名すると有効になります。そのノードで \(command) を実行してください。"
        }
    }
}
