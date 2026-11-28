@testable import AgencyModel
import Testing

@Suite("The sending tray's words")
struct OutboxWordingTests {
    @Test("they are the web's (#446), and a reason is shown as given")
    func words() {
        #expect(Ja.Outbox.heading(count: 3) == "送信中の決定（3）")
        #expect(Ja.Outbox.sending == "送信中…")
        #expect(Ja.Outbox.waiting(nextInSeconds: 0) == "まだ送れていません。まもなく再送します。")
        #expect(Ja.Outbox.waiting(nextInSeconds: 1) == "まだ送れていません。まもなく再送します。")
        #expect(Ja.Outbox.waiting(nextInSeconds: 15) == "まだ送れていません。次の再送は 1 分以内 です。")
        #expect(Ja.Outbox.waiting(nextInSeconds: 300) == "まだ送れていません。次の再送は 5 分後 です。")
        #expect(Ja.Outbox.failed("the team's node refused: not yours") == "送れませんでした: the team's node refused: not yours")
        #expect(Ja.Outbox.sendAgain == "もう一度送る" && Ja.Outbox.discard == "破棄")
        #expect(Ja.Outbox.discardExplain == "破棄すると、書いた内容つきでリクエストが山に戻ります。")
        #expect(Ja.Outbox.more(2) == "ほか 2 件" && Ja.Outbox.shown == 4)
        #expect(Ja.Outbox.notKept == "この端末に決定を残せなかったため、送っていません。もう一度お試しください。")
        #expect(Ja.Outbox.signInEnded == "サインインが切れています。サインインし直すと、続きを送ります。")
        #expect(Ja.Recent.chose("承認する") == "「承認する」を選びました。", "the log's own sentence")
    }
}
