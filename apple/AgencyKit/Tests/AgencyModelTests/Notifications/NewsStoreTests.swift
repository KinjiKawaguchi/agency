@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakeNews: NewsSource, @unchecked Sendable {
    private let lock = NSLock()
    var me: Result<Me, Error> = .success(Fixture.decode(Me.self, "me"))
    var snapshot: Result<TeamSnapshot, Error> = .success(Teams.snapshot())
    var devices: Result<DeviceList, Error> = .success(Fixture.decode(DeviceList.self, "device-list"))
    var decisions: Result<DecisionLog, Error> = .success(Fixture.decode(DecisionLog.self, "decision-log"))

    func me() async throws -> Me { try lock.withLock { try me.get() } }
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot { try lock.withLock { try snapshot.get() } }
    func devices() async throws -> DeviceList { try lock.withLock { try devices.get() } }
    func decisions(limit: Int) async throws -> DecisionLog { try lock.withLock { try decisions.get() } }
}

private func deviceList(requests: [(label: String, key: String)]) -> DeviceList {
    let reqs = requests.map { #"{"deviceId":"d-\#($0.key)","label":"\#($0.label)","pairingCode":"a652-124a-fda7","publicKey":"\#($0.key)","requestedAt":"2026-11-24T12:00:00Z"}"# }
    return try! JSONDecoder().decode(DeviceList.self, from: Data(#"{"orgs":[],"requests":[\#(reqs.joined(separator: ","))]}"#.utf8))
}

private func log(_ entries: [(id: String, state: String, title: String?)]) -> DecisionLog {
    let items = entries.map { entry -> String in
        let state = entry.state == "refused" ? #"{"kind":"refused","message":"no"}"# : #"{"kind":"\#(entry.state)"}"#
        let title = entry.title.map { #","title":"\#($0)""# } ?? ""
        return #"{"id":"\#(entry.id)","org":"acme","team":"app","kind":"answer","target":"r-1","text":"","decidedAt":"2026-11-24T12:00:00Z","state":\#(state)\#(title)}"#
    }
    return try! JSONDecoder().decode(DecisionLog.self, from: Data(#"{"entries":[\#(items.joined(separator: ","))]}"#.utf8))
}

@MainActor
@Suite("NewsStore")
struct NewsStoreTests {
    private let source = FakeNews()
    private let sent = SentDecisions()
    private let team = Teams.snapshot()

    private func store(canApprove: Bool = true) -> NewsStore {
        let sent = sent
        return NewsStore(source: source) {
            NotificationOwn(deviceKey: "own-key", canApprove: canApprove, sentDecisionIds: sent.ids)
        }
    }

    private func newRequest(_ id: String = "r-new", title: String = "Deploy now?") -> [String: Any] {
        Teams.request(id: id, to: nil, title: title)
    }

    @Test("the first reading is a baseline: nothing in it is news")
    func baseline() async {
        let news = store()
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("a request that now waits on the viewer makes a banner with its title, which opens it on the desk")
    func newRequest() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(addRequests: [newRequest()]))
        await news.poll()
        #expect(news.banners.count == 1)
        let banner = news.banners[0]
        #expect(banner.kind == .request && banner.count == 1)
        #expect(banner.text.contains("Deploy now?"))
        #expect(banner.text.hasSuffix("があなたを待っています"))
        #expect(banner.destination == .desk(itemID: "acme:app:r-new"))
    }

    @Test("several new requests are one banner that says how many")
    func severalRequests() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(addRequests: [newRequest("r-1x"), newRequest("r-2x"), newRequest("r-3x")]))
        await news.poll()
        #expect(news.banners.map(\.text) == ["3 件の依頼があなたを待っています"])
        #expect(news.banners[0].count == 3)
    }

    @Test("the same reading twice makes no banner")
    func sameTwice() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(addRequests: [newRequest()]))
        await news.poll()
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("a team that cannot be read is no news, and the read after it is not all new")
    func failedRead() async {
        let news = store()
        await news.poll()
        source.snapshot = .failure(NodeError.http(status: 503, message: "not ready"))
        await news.poll()
        #expect(news.banners.isEmpty)
        source.snapshot = .success(Teams.snapshot())
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("an account that cannot be read makes no banner and breaks nothing")
    func accountFails() async {
        source.me = .failure(NodeError.sessionExpired)
        let news = store()
        await news.poll()
        #expect(news.banners.isEmpty)
        source.me = .success(Fixture.decode(Me.self, "me"))
        await news.poll()
        #expect(news.banners.isEmpty, "the first good reading is the baseline")
    }

    @Test("a kind that is turned off makes no banner, and its events are not announced later")
    func disabled() async {
        let news = store()
        await news.poll()
        news.enabledKinds.remove(.request)
        source.snapshot = .success(Teams.snapshot(addRequests: [newRequest()]))
        await news.poll()
        #expect(news.banners.isEmpty)
        news.enabledKinds.insert(.request)
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("a kind the visible screen already shows makes no banner")
    func suppressed() async {
        source.devices = .success(deviceList(requests: []))
        let news = store()
        await news.poll()
        news.suppressedKinds = [.device]
        source.devices = .success(deviceList(requests: [("Tablet", "k-tablet")]))
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("a device that asks to be added makes a banner with its label, which opens the devices screen")
    func deviceBanner() async {
        source.devices = .success(deviceList(requests: []))
        let news = store()
        await news.poll()
        source.devices = .success(deviceList(requests: [("Tablet", "k-tablet"), ("own", "own-key")]))
        await news.poll()
        #expect(news.banners.map(\.text) == ["端末「Tablet」が追加を待っています"])
        #expect(news.banners[0].destination == .devices)
    }

    @Test("a phone that cannot approve is told nothing about devices")
    func cannotApprove() async {
        source.devices = .success(deviceList(requests: []))
        let news = store(canApprove: false)
        await news.poll()
        source.devices = .success(deviceList(requests: [("Tablet", "k-tablet")]))
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("a refused decision makes a banner with its title; a success this phone sent does not")
    func decisions() async {
        source.decisions = .success(log([]))
        let news = store()
        await news.poll()
        sent.record("mine1")
        source.decisions = .success(log([("mine1", "applied", "Ship it?"), ("other1", "refused", "Drop it?")]))
        await news.poll()
        #expect(news.banners.map(\.text) == ["「Drop it?」の決定は断られました"])
        #expect(news.banners[0].destination == .desk(itemID: nil))
    }

    @Test("a decision without a title still says what happened")
    func untitled() async {
        source.decisions = .success(log([]))
        let news = store()
        await news.poll()
        source.decisions = .success(log([("c1", "refused", nil)]))
        await news.poll()
        #expect(news.banners.map(\.text) == ["「(題名なし)」の決定は断られました"])
    }

    @Test("a participant who now waits on the viewer makes a banner with their name, which opens the people screen")
    func participant() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(waitingOnViewer: ["backend", "frontend"]))
        await news.poll()
        #expect(news.banners.map(\.kind) == [.participant])
        #expect(news.banners[0].text == "Frontend があなたの返事を待っています")
        #expect(news.banners[0].destination == .people)
    }

    @Test("several participants are counted as participants, in the words of the glossary")
    func severalParticipants() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(waitingOnViewer: ["backend", "frontend", "devops"]))
        await news.poll()
        #expect(news.banners.map(\.text) == ["参加者 2 人があなたの返事を待っています"])
    }

    @Test("a work item newly blocked makes a banner with its title and the reason; several say how many")
    func blocked() async {
        let news = store()
        await news.poll()
        let item = { (ref: String, title: String) -> [String: Any] in
            ["ref": ref, "title": title, "status": "next", "blocked": ["reason": "needsDecision", "since": "2026-11-23"]]
        }
        source.snapshot = .success(Teams.snapshot(addWork: [item("app#31", "Migrate the store")]))
        await news.poll()
        #expect(news.banners.map(\.kind) == [.blocked])
        #expect(news.banners[0].text.contains("Migrate the store") && news.banners[0].text.contains("判断待ち"))
        #expect(news.banners[0].destination == .people)
        source.snapshot = .success(Teams.snapshot(addWork: [item("app#31", "Migrate the store"), item("app#32", "Two"), item("app#33", "Three")]))
        await news.poll()
        #expect(news.banners.map(\.text) == ["2 件の仕事が止まりました"])
    }

    @Test("a new work item that is not blocked is no news")
    func unblockedWork() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(addWork: [["ref": "app#40", "title": "Free work", "status": "next"]]))
        await news.poll()
        #expect(news.banners.isEmpty)
    }

    @Test("at most three banners a reading, in the order of the kinds")
    func atMostThree() async {
        source.devices = .success(deviceList(requests: []))
        source.decisions = .success(log([]))
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(waitingOnViewer: ["backend", "frontend"], addRequests: [newRequest()]))
        source.devices = .success(deviceList(requests: [("Tablet", "k-tablet")]))
        source.decisions = .success(log([("x1", "refused", "Nope")]))
        await news.poll()
        #expect(news.banners.map(\.kind) == [.request, .participant, .device])
    }

    @Test("dismissing takes the banners away")
    func dismiss() async {
        let news = store()
        await news.poll()
        source.snapshot = .success(Teams.snapshot(addRequests: [newRequest()]))
        await news.poll()
        #expect(!news.banners.isEmpty)
        news.dismiss()
        #expect(news.banners.isEmpty)
    }
}

@Suite("SentDecisions")
struct SentDecisionsTests {
    @Test("it remembers the ids it was given")
    func remembers() {
        let sent = SentDecisions()
        #expect(sent.ids.isEmpty)
        sent.record("a")
        sent.record("a")
        sent.record("b")
        #expect(sent.ids == ["a", "b"])
    }
}
