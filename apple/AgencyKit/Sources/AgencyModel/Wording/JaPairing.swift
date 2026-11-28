import Foundation

extension Ja {
    /// What a phone that asked to be added tells its owner: how it gets approved, and why the codes are compared.
    /// The web client says the same (agency#157).
    public enum Pairing {
        public static let waitingHeading = "この端末の追加を待っています"
        public static let codeCaption = "照合コード"
        public static let compare = "どちらの画面のコードも同じか確かめてください。1 文字でも違えば承認しないでください。違うコードを承認すると、知らない端末を追加してしまいます。"
        public static let autoUpdate = "承認されると、この画面は自動で切り替わります。"
        public static let checkNow = "今すぐ調べる"

        /// Approve it from a device the person already has, named by its label.
        public static func fromDevices(_ labels: [String]) -> String {
            let places = labels.count == 1 ? labels[0] : labels.joined(separator: "、") + " のどれか"
            return "\(places)で Agency を開き、「端末」の画面で、このコードのリクエストを承認してください。"
        }

        /// The account has no device yet: the node's machine approves the first one.
        public static func atTheNode(_ command: String) -> String {
            "ノードを動かしているマシンで、「\(command)」を実行し、表示されたコードを確かめて承認してください。"
        }

        public static func instruction(_ approval: ApprovalWay) -> String {
            switch approval {
            case .fromDevices(let labels): fromDevices(labels)
            case .atTheNode(let command): atTheNode(command)
            }
        }
    }
}
