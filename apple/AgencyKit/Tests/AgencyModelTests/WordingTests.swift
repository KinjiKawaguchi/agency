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
        #expect(DeskGroup.allCases.map(Ja.Desk.group) == ["決裁する稟議", "あなたへの質問", "ボードで判断待ちのもの", "ほかの人の返事待ち"])
        #expect([Request.Kind.proposal, .question, .pendingDecision].map(Ja.Paper.verb) == ["決裁する", "答える", "判断を送る"])
        #expect([Request.Kind.proposal, .question, .pendingDecision].map(Ja.Paper.verbDone) == ["決裁しました", "答えました", "判断を送りました"])
        #expect(Ja.Paper.sendWith("決裁する", choice: "承認") == "「承認」で決裁する")
        #expect(Ja.Paper.sentComment(to: "Lead", kind: .proposal) == "コメントしました。Lead に届きます。まだ決裁はされていません。")
        #expect(Ja.Paper.sentComment(to: "Lead", kind: .question) == "コメントしました。Lead に届きます。まだ決まっていません。")
        #expect(Ja.Paper.commentExplain(.proposal).hasPrefix("決裁はしません。"))
        #expect(Ja.Paper.commentExplain(.question).hasPrefix("決めません。"))
        #expect(!Ja.Paper.commentExplain(.question).contains("決裁"))
        #expect(!Ja.Paper.commentExplain(.pendingDecision).contains("決裁"))
    }

    @Test("the tab names are the web client's navigation labels")
    func tabs() {
        #expect([Ja.Tabs.desk, Ja.Tabs.overview, Ja.Tabs.people, Ja.Tabs.costs, Ja.Tabs.devices, Ja.Tabs.orgChart]
            == ["決めること", "全体", "参加者と仕事", "費用", "端末", "組織図"])
    }

    @Test("a queued decision says how often it was tried and when it goes next")
    func queued() {
        #expect(Ja.Recent.queuedDetail(attempts: 0, retry: nil) == "まだ送れていません。まもなく再送します。")
        #expect(Ja.Recent.queuedDetail(attempts: 2, retry: "3 分後") == "2 回試しました。次の再送は 3 分後 です。")
    }

    @Test("the words never explain a keyboard", arguments: [
        Ja.Paper.commentExplain(.proposal), Ja.Paper.commentExplain(.question), Ja.Paper.notePlaceholderWithOptions, Ja.Paper.faceIDNote, Ja.Desk.empty,
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
