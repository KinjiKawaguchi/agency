extension Ja {
    /// What the owner is told when the app and the home node no longer fit each other.
    public enum Compatibility {
        public static let nodeTooOld = "ホームノードが古いため、このアプリは通信できません。ノードを更新してください。"
        /// For a guest, who cannot update the node: whoever runs it can.
        public static let nodeTooOldForGuest = "ホームノードが古いため、このアプリは通信できません。ノードの管理者に伝えてください。"
        public static let appTooOld = "アプリが古いため、ホームノードと通信できません。アプリを更新してください。"
    }
}
