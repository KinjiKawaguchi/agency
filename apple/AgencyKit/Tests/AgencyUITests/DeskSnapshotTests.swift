#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The desk drawn at phone width. Each test checks the one rule the owner asked for (nothing is wider
/// than the screen) and, when `AGENCY_SNAPSHOT_DIR` is set, leaves a PNG to look at.
@MainActor
@Suite("Desk renderings", .serialized)
struct DeskSnapshotTests {
    private let now = UIFixtures.now
    private let width = Snapshot.phoneWidth

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == width, "\(name) is \(result.size.width) points wide, the screen is \(width)", sourceLocation: sourceLocation)
        #expect(result.size.height > 40, sourceLocation: sourceLocation)
    }

    // MARK: The pile

    private var pileTeam: Team {
        UIFixtures.team(extra: [
            UIFixtures.request(
                id: "r-long", kind: "proposal", title: UIFixtures.longJapaneseTitle, blocks: ["backend", "lead"],
                options: [UIFixtures.option("approve", "承認する"), UIFixtures.option("reject", "却下する")]
            ),
            UIFixtures.request(id: "r-unbroken", title: "Merge \(UIFixtures.unbrokenID) into main?", from: UIFixtures.unbrokenID),
            UIFixtures.request(id: "r-others", title: "Should the nightly export include archived projects?", to: "frontend"),
        ], mine: [Teams.q, Teams.p, Teams.d, "r-long", "r-unbroken"])
    }

    @Test("the pile, with a very long title and an id with no spaces, fits the screen in light and dark")
    func pile() {
        let team = pileTeam
        let items = Desk.openItems([team])
        let view = DeskPile(items: items, situation: Situation(teams: [team], items: items), sent: ["acme:app:r-unbroken"], now: now, isStatic: true)
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("desk-pile", view, scheme: scheme), "the pile (\(scheme))")
        }
    }

    @Test("an empty desk says there is nothing to decide")
    func emptyPile() {
        let team = UIFixtures.team(extra: [], replacing: true)
        let view = DeskPile(items: [], situation: Situation(teams: [team], items: []), sent: [], now: now, isStatic: true)
        expectFits(Snapshot.capture("desk-empty", view), "the empty desk")
    }

    // MARK: A request open

    private func detail(
        _ item: DeskItem, draft: DeskDraft = DeskDraft(), status: ReplyStatus = .idle
    ) -> RequestDetail {
        RequestDetail(item: item, draft: draft, status: status, now: now, isStatic: true, onText: { _ in }, onPick: { _ in }, onDecide: {}, onComment: {})
    }

    @Test("a question with options: none picked, one picked with a note")
    func questionWithOptions() {
        let team = UIFixtures.team()
        let item = UIFixtures.item(Teams.q, in: team)
        expectFits(Snapshot.capture("detail-options", detail(item)), "a question with options")
        let picked = detail(item, draft: DeskDraft(text: "インデックスを付けてからお願いします。", optionKey: "b"))
        expectFits(Snapshot.capture("detail-options-picked", picked), "a question with an option picked")
    }

    @Test("a proposal with a document, in light and dark")
    func proposal() {
        let team = UIFixtures.team(extra: [
            UIFixtures.request(
                id: "r-doc", kind: "proposal", title: "クラウド移行計画の承認", context: "来期の基盤について、三段階で移す案です。",
                document: UIFixtures.document, blocks: ["backend"],
                options: [
                    UIFixtures.option("approve", "承認する", recommended: true, detail: "費用は据え置き。切り戻し手順あり。"),
                    UIFixtures.option("reject", "却下する"),
                ],
                thread: [["from": "lead", "text": "前回は予算の件で差し戻しました。", "at": "2026-11-23T09:00:00Z"]]
            ),
        ])
        let item = UIFixtures.item("r-doc", in: team)
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("detail-proposal", detail(item, draft: DeskDraft(optionKey: "approve")), scheme: scheme), "the proposal (\(scheme))")
        }
    }

    @Test("the comment path: text typed, no option picked, the comment button and what it means")
    func commentPath() {
        let team = UIFixtures.team()
        let item = UIFixtures.item(Teams.q, in: team)
        let view = detail(item, draft: DeskDraft(text: "決める前に、移行にかかる時間を教えてください。"))
        expectFits(Snapshot.capture("detail-comment", view), "the comment path")
    }

    @Test("a very long title and long unbroken text wrap and never widen the screen")
    func longTitle() {
        let team = UIFixtures.team(extra: [
            UIFixtures.request(
                id: "r-long", kind: "proposal", title: UIFixtures.longJapaneseTitle + UIFixtures.unbrokenID,
                context: "Branch \(UIFixtures.unbrokenID) waits on \(UIFixtures.unbrokenID)",
                blocks: ["backend"], options: [UIFixtures.option("go", "承認 " + UIFixtures.unbrokenID, detail: UIFixtures.unbrokenID)]
            ),
        ])
        let item = UIFixtures.item("r-long", in: team)
        for scheme in [ColorScheme.light, .dark] {
            expectFits(Snapshot.capture("detail-long", detail(item, draft: DeskDraft(text: UIFixtures.unbrokenID, optionKey: "go")), scheme: scheme), "the long title (\(scheme))")
        }
    }

    @Test("a request for someone else can be read and not answered")
    func notMine() {
        let team = UIFixtures.team(extra: [UIFixtures.request(id: "r-other", title: "Should the export include archives?", to: "frontend", options: [UIFixtures.option("y", "Yes")])], mine: [])
        let item = UIFixtures.item("r-other", in: team)
        expectFits(Snapshot.capture("detail-not-mine", detail(item)), "a request for someone else")
    }

    @Test("a decision that is open with no options is answered in words")
    func answerInWords() {
        let team = UIFixtures.team()
        let item = UIFixtures.item(Teams.d, in: team)
        expectFits(Snapshot.capture("detail-answer", detail(item, draft: DeskDraft(text: "旧形式の書き出しは来月で終了します。"))), "an answer in words")
    }

    @Test("sending, sent, queued, and failed")
    func statuses() {
        let team = UIFixtures.team()
        let item = UIFixtures.item(Teams.q, in: team)
        let draft = DeskDraft(text: "後で連絡します。")
        expectFits(Snapshot.capture("detail-sending", detail(item, draft: draft, status: .sending)), "sending")
        expectFits(Snapshot.capture("detail-sent-comment", detail(item, status: .sent(SentReply(isComment: true, outcome: .applied, message: nil)))), "sent comment")
        expectFits(Snapshot.capture("detail-queued", detail(item, status: .sent(SentReply(isComment: false, outcome: .queued, message: "asleep")))), "queued")
        expectFits(Snapshot.capture("detail-failed", detail(item, draft: draft, status: .failed("writes are for the owner"))), "failed")
    }

    // MARK: Recent decisions

    @Test("recent decisions: a queued one with its attempts, and the Japanese comment of the fixture")
    func recent() {
        let queued = UIFixtures.decisionLog("decision-log-member").entries
        let applied = UIFixtures.decisionLog("decision-log").entries
        let view = RecentDecisions(entries: queued + applied, failure: nil, now: now)
        expectFits(Snapshot.capture("recent", view), "recent decisions")
    }
}

/// The ids of the shared sample's three requests.
enum Teams {
    static let q = "r-20261124-110000-a1b2"
    static let p = "r-20261124-113000-c3d4"
    static let d = "app#14"
}
#endif
