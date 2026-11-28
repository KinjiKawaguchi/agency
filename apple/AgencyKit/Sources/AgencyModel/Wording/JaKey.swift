import Foundation

/// How this device's key asks the person to confirm each signature: the iPhone with Face ID, the Mac with
/// Touch ID or the account's password (the key is the Secure Enclave's on both; the Mac has no rule that
/// needs a biometric, because a Mac may have none, or be used closed).
public enum KeyProtection: Sendable {
    case faceID
    case touchIDOrPassword
}

extension Ja {
    /// What the screens say of the key and how it is confirmed.
    public enum Key {
        /// What the person confirms with, as the start of "…確認します" ("Face ID で", "Touch ID か Mac のパスワードで"); a Latin-script name takes a space before the particle, katakana does not.
        static func by(_ protection: KeyProtection) -> String {
            switch protection {
            case .faceID: "Face ID で"
            case .touchIDOrPassword: "Touch ID か Mac のパスワードで"
            }
        }

        /// The symbol for the confirmation.
        public static func symbol(_ protection: KeyProtection) -> String {
            switch protection {
            case .faceID: "faceid"
            case .touchIDOrPassword: "touchid"
            }
        }

        public static func confirmSend(_ protection: KeyProtection) -> String { "送るときに \(by(protection))確認します。" }

        public static func confirmAdd(_ protection: KeyProtection) -> String { "追加するときに \(by(protection))確認します。" }

        public static func resetExplain(_ protection: KeyProtection) -> String {
            switch protection {
            case .faceID:
                "Face ID の登録を変えたあとで署名できなくなったときに使います。作り直すと、この端末をもう一度追加する必要があります。"
            case .touchIDOrPassword:
                "鍵を使えなくなったときに使います。作り直すと、この Mac をもう一度追加する必要があります。"
            }
        }

        /// The screen for a key that is stored and cannot be used now.
        public enum Unusable {
            public static let title = "この端末の鍵を使えません"
            public static let explain = "決定に署名する鍵は端末に残っていますが、いまは使えません。端末がロックされている、Face ID の登録を変えた、などが考えられます。鍵は自動では作り直しません。"
            public static func reason(_ text: String) -> String { "理由: \(text)" }
            public static let tryAgain = "もう一度試す"
            public static let makeExplain = "使えない鍵は残したまま、新しい鍵を作って、この端末をもう一度追加します。追加するまで、この端末では決めることも、コメントすることもできません。"
        }

        public static func noKey(_ protection: KeyProtection) -> String {
            switch protection {
            case .faceID: "この端末では、決定に署名する鍵を作れません。Face ID を設定してから、アプリを開き直してください。"
            case .touchIDOrPassword: "この Mac では、決定に署名する鍵を作れません。Touch ID かパスワードを使える状態にしてから、アプリを開き直してください。"
            }
        }
    }
}
