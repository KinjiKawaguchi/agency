import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Wording")
struct WordingTests {
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    @Test("how long ago, as the web client says it")
    func ago() {
        func ago(_ seconds: Double) -> String { Ago.text(from: now.addingTimeInterval(-seconds), now: now) }
        #expect(ago(0) == "たった今")
        #expect(ago(59) == "たった今")
        #expect(ago(60) == "1 分前")
        #expect(ago(59 * 60) == "59 分前")
        #expect(ago(3600) == "1 時間前")
        #expect(ago(47 * 3600) == "47 時間前")
        #expect(ago(2 * 86_400) == "2 日前")
        #expect(ago(10 * 86_400) == "10 日前")
        #expect(Ago.text(from: now.addingTimeInterval(300), now: now) == "5 分後")
    }

    @Test("the desk's group names and the three request kinds")
    func names() {
        #expect(DeskGroup.allCases.map(Ja.Desk.group) == ["決裁する稟議", "あなたへの質問", "ボードで相談待ちのもの", "このアプリが知らない種類のリクエスト", "ほかの人の返事待ち"])
        #expect([Request.Kind.proposal, .question, .pendingDecision].map(Ja.Paper.verb) == ["決裁する", "答える", "判断を送る"])
        #expect(Ja.Paper.sendWith("決裁する", choice: "承認") == "「承認」で決裁する")
        #expect([Request.Kind.proposal, .question, .pendingDecision, .unknown].map(Ja.Paper.type) == ["稟議", "質問", "相談", "リクエスト"], "the kinds' names are the glossary's")
        #expect(Ja.Paper.commentExplain(.proposal).hasPrefix("決裁はしません。"))
        #expect(Ja.Paper.commentExplain(.question).hasPrefix("決めません。"))
        #expect(!Ja.Paper.commentExplain(.question).contains("決裁"))
        #expect(!Ja.Paper.commentExplain(.pendingDecision).contains("決裁"))
    }

    @Test("the tab names are the web client's navigation labels")
    func tabs() {
        #expect([Ja.Tabs.desk, Ja.Tabs.overview, Ja.Tabs.people, Ja.Tabs.costs, Ja.Tabs.devices, Ja.Tabs.orgChart]
            == ["あなた宛てのリクエスト", "チームの状況", "メンバーの作業状況", "コスト", "端末", "組織図"])
    }

    @Test("the pairing card says how the device gets approved, by labels or by the node's command")
    func pairing() {
        #expect(Ja.Pairing.instruction(.fromDevices(labels: ["MacBook"])) == "MacBookで Agency を開き、「端末」の画面で、このコードのリクエストを承認してください。")
        #expect(Ja.Pairing.instruction(.fromDevices(labels: ["MacBook", "iPad"])) == "MacBook、iPad のどれかで Agency を開き、「端末」の画面で、このコードのリクエストを承認してください。")
        #expect(Ja.Pairing.instruction(.atTheNode(command: "agency device-add --dir /srv/agency"))
            == "ノードを動かしているマシンで、「agency device-add --dir /srv/agency」を実行し、表示されたコードを確かめて承認してください。")
        #expect(Ja.Pairing.compare.contains("1 文字でも違えば承認しないでください"))
        #expect(Ja.Pairing.autoUpdate.contains("自動"))
    }

    @Test("a queued decision says how often it was tried and when it goes next")
    func queued() {
        #expect(Ja.Recent.queuedDetail(attempts: 0, retry: nil) == "まだ届いていません。まもなく再送します。")
        #expect(Ja.Recent.queuedDetail(attempts: 2, retry: "3 分後") == "2 回試しました。次の再送は 3 分後 です。")
    }

    @Test("the words never explain a keyboard", arguments: [
        Ja.Paper.commentExplain(.proposal), Ja.Paper.commentExplain(.question), Ja.Paper.notePlaceholderWithOptions, Ja.Key.confirmSend(.faceID), Ja.Key.confirmSend(.touchIDOrPassword), Ja.Desk.empty,
    ])
    func noKeyboardHints(text: String) {
        for word in ["キー", "ショートカット", "Enter", "Cmd", "Ctrl", "⌘"] {
            #expect(!text.contains(word))
        }
    }
}

@Suite("MarkdownBlock")
struct MarkdownBlockTests {
    @Test("headings, paragraphs, lists, quotes and code")
    func blocks() {
        let document = """
        # Plan
        Two lines
        of text.

        - first
        * second
        1. one
        12. twelve
        > quoted
        ```
        let x = 1
        ```
        ## Next
        """
        #expect(MarkdownBlock.parse(document) == [
            .heading(level: 1, text: "Plan"),
            .paragraph("Two lines\nof text."),
            .bullet("first"), .bullet("second"),
            .numbered(number: 1, text: "one"), .numbered(number: 12, text: "twelve"),
            .quote("quoted"),
            .code("let x = 1"),
            .heading(level: 2, text: "Next"),
        ])
    }

    @Test("raw HTML, links and images stay text; nothing is interpreted")
    func rawHTML() {
        let blocks = MarkdownBlock.parse("<script>alert(1)</script>\n[a](https://evil.example) ![i](https://evil.example/x.png)")
        #expect(blocks == [.paragraph("<script>alert(1)</script>\n[a](https://evil.example) ![i](https://evil.example/x.png)")])
    }

    @Test("a hash without a space, a dash without a space and a number without a dot are text")
    func notBlocks() {
        #expect(MarkdownBlock.parse("#tag\n-5 degrees\n2024 was") == [.paragraph("#tag\n-5 degrees\n2024 was")])
        #expect(MarkdownBlock.parse("####### seven") == [.paragraph("####### seven")])
    }

    @Test("two backticks are inline code, not a fence; and a number too long for an Int is text")
    func inlineAndHuge() {
        #expect(MarkdownBlock.parse("``code`` first") == [.paragraph("``code`` first")])
        #expect(MarkdownBlock.parse("99999999999999999999. text") == [.paragraph("99999999999999999999. text")])
    }

    @Test("an unclosed code fence is closed at the end, and CRLF and Japanese text are kept")
    func edges() {
        #expect(MarkdownBlock.parse("```\r\nabc") == [.code("abc")])
        #expect(MarkdownBlock.parse("# 見出し\r\n本文です。") == [.heading(level: 1, text: "見出し"), .paragraph("本文です。")])
        #expect(MarkdownBlock.parse("") == [])
    }
}
