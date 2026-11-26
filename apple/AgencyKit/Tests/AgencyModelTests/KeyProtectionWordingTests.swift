@testable import AgencyModel
import Testing

@Suite("Key protection wording")
struct KeyProtectionWordingTests {
    @Test("the iPhone says Face ID, in the words the screens always used")
    func iPhone() {
        let protection = KeyProtection.faceID
        #expect(Ja.Key.confirmSend(protection) == "送るときに Face ID で確認します。")
        #expect(Ja.Key.confirmAdd(protection) == "追加するときに Face ID で確認します。")
        #expect(Ja.Key.resetExplain(protection) == "Face ID の登録を変えたあとで署名できなくなったときに使います。作り直すと、この端末をもう一度追加する必要があります。")
        #expect(Ja.Key.noKey(protection) == "この端末では、決定に署名する鍵を作れません。Face ID を設定してから、アプリを開き直してください。")
        #expect(Ja.Key.symbol(protection) == "faceid")
    }

    @Test("the Mac says Touch ID or the Mac's password, and calls itself a Mac")
    func mac() {
        let protection = KeyProtection.touchIDOrPassword
        #expect(Ja.Key.confirmSend(protection) == "送るときに Touch ID か Mac のパスワードで確認します。")
        #expect(Ja.Key.confirmAdd(protection) == "追加するときに Touch ID か Mac のパスワードで確認します。")
        #expect(Ja.Key.resetExplain(protection) == "鍵を使えなくなったときに使います。作り直すと、この Mac をもう一度追加する必要があります。")
        #expect(!Ja.Key.resetExplain(protection).contains("登録"), "with the password as an alternative, a change of fingerprints does not break the key")
        #expect(Ja.Key.noKey(protection).hasPrefix("この Mac では"))
        #expect(Ja.Key.symbol(protection) == "touchid")
    }
}
