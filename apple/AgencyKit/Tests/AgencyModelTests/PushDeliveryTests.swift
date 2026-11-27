@testable import AgencyAPI
@testable import AgencyModel
import AgencyProtocol
import Foundation
import Testing

@Suite("Push delivery")
struct PushDeliveryTests {
    private let tokyo = TimeZone(identifier: "Asia/Tokyo")!

    private func token(_ tail: String = "ab12cd34", taken: String? = nil, failed: String? = nil, reason: String = "APNs answered 503") throws -> RegisteredPushToken {
        let taken = taken.map { #","lastSucceededAt":"\#($0)""# } ?? ""
        let failed = failed.map { #","lastFailure":{"at":"\#($0)","reason":"\#(reason)"}"# } ?? ""
        let json = #"{"token":"\#(tail)","environment":"production","label":"iPhone","events":["request"],"registeredAt":"2026-11-20T09:00:00Z"\#(taken)\#(failed)}"#
        return try JSONDecoder().decode(RegisteredPushToken.self, from: Data(json.utf8))
    }

    private func at(_ text: String) -> ReportedTime { ReportedTime(text: text) }

    @Test("nothing known, a taking, a refusal with none taken, a refusal later than a taking, and a taking later than a refusal")
    func rule() throws {
        #expect(PushDelivery(token: try token()) == .unknown)
        #expect(PushDelivery(token: try token(taken: "2026-11-26T09:00:00Z")) == .taken(at("2026-11-26T09:00:00Z")))
        #expect(PushDelivery(token: try token(failed: "2026-11-26T09:00:00Z")) == .failing(at: at("2026-11-26T09:00:00Z"), reason: "APNs answered 503", lastTakenAt: nil))
        #expect(PushDelivery(token: try token(taken: "2026-11-26T08:00:00Z", failed: "2026-11-26T09:00:00Z"))
            == .failing(at: at("2026-11-26T09:00:00Z"), reason: "APNs answered 503", lastTakenAt: at("2026-11-26T08:00:00Z")))
        #expect(PushDelivery(token: try token(taken: "2026-11-26T10:00:00Z", failed: "2026-11-26T09:00:00Z")) == .taken(at("2026-11-26T10:00:00Z")))
    }

    @Test("a taking at the same moment as a refusal, or the same moment written in another zone, means the notices go through")
    func equal() throws {
        #expect(PushDelivery(token: try token(taken: "2026-11-26T09:00:00Z", failed: "2026-11-26T09:00:00Z")).isFailing == false)
        #expect(PushDelivery(token: try token(taken: "2026-11-26T18:00:00+09:00", failed: "2026-11-26T09:00:00Z")).isFailing == false, "times are compared as times, not as texts")
        #expect(PushDelivery(token: try token(taken: "2026-11-26T09:30:00+09:00", failed: "2026-11-26T09:00:00Z")).isFailing, "an offset is not a later time")
    }

    @Test("a refusal says when and why, as the node gives it, and when the last notice was taken or that none was, in the web's words")
    func words() throws {
        let failing = PushDelivery(token: try token(taken: "2026-11-26T08:00:00Z", failed: "2026-11-26T09:00:00Z", reason: "APNs refused the provider token (ExpiredProviderToken)"))
        #expect(failing.lines(in: tokyo) == [
            "この端末へのプッシュ通知を、APNs は最後に受け付けませんでした（11月26日 18:00）: APNs refused the provider token (ExpiredProviderToken)。ノードは最大 1 日のあいだ再送します。失敗が続くときは、端末の通知の設定と、ノードの APNs の設定を確かめてください。",
            "最後に受け付けた通知は 11月26日 17:00 のものです。",
        ])
        let never = PushDelivery(token: try token(failed: "2026-11-26T09:00:00Z"))
        #expect(never.lines(in: tokyo)?.last == "まだ 1 件も受け付けていません。", "never is a missing value, not a date")
        #expect(never.isFailing)
    }

    @Test("a taking says nothing, and so does a token the node lists with nothing to say: that is not a refusal (an older node records none, a new phone has been sent none)")
    func quiet() throws {
        #expect(PushDelivery.taken(at("2026-11-26T09:00:00Z")).lines() == nil)
        #expect(PushDelivery.unknown.lines() == nil)
        #expect(!PushDelivery.unknown.isFailing)
    }

    @Test("a time that cannot be read is not a later time: the refusal stands, and is said with the text the node wrote")
    func unreadable() throws {
        let both = PushDelivery(token: try token(taken: "yesterday", failed: "2026-11-26T09:00:00Z"))
        #expect(both.isFailing, "a taking whose time cannot be compared does not hide a refusal")
        let badFailure = PushDelivery(token: try token(taken: "2026-11-26T09:00:00Z", failed: "this morning"))
        #expect(badFailure.isFailing)
        #expect(badFailure.lines(in: tokyo)?.first?.contains("（this morning）") == true, "the node's text where a time cannot be formatted")
        #expect(PushDelivery(token: try token(taken: "now", failed: nil)) == .taken(at("now")))
    }

    @Test("the node's reason and the text of a time that cannot be read are shown without hidden characters")
    func shown() throws {
        let delivery = PushDelivery(token: try token(taken: "2026-11-26T09:00:00Z", failed: "now\u{202E}", reason: "APNs answered\u{202E} 503\u{200B}"))
        guard case .failing(let at, let reason, _) = delivery else { Issue.record("should be failing"); return }
        #expect(reason == "APNs answered? 503?" && at.text == "now?")
        #expect(delivery.lines(in: tokyo)?.first?.contains("（now?）: APNs answered? 503?。") == true)
        #expect(!(delivery.lines(in: tokyo) ?? []).joined().unicodeScalars.contains { NodeLabel.isHidden($0) })
    }

    @Test("this phone's token is found among the listed ones by its last 8 characters, in either case, and a token not listed is nothing")
    func own() throws {
        let listed = [try token("11112222"), try token("ab12cd34", failed: "2026-11-26T09:00:00Z")]
        #expect(PushDelivery.ofOwn(token: "0f0e0d0c0b0a09080706050403020100ab12cd34", in: listed)?.isFailing == true)
        #expect(PushDelivery.ofOwn(token: "0F0E0D0C0B0A09080706050403020100AB12CD34", in: listed)?.isFailing == true)
        #expect(PushDelivery.ofOwn(token: "00000000000000000000000000000000ffffffff", in: listed) == nil)
        #expect(PushDelivery.ofOwn(token: "ab12cd34", in: []) == nil)
    }
}

private final class ScriptedTokens: PushTokenSource, @unchecked Sendable {
    private let lock = NSLock()
    private var _result: Result<[RegisteredPushToken], Error>
    private var _reads = 0
    init(_ result: Result<[RegisteredPushToken], Error>) { _result = result }
    var reads: Int { lock.withLock { _reads } }
    func set(_ result: Result<[RegisteredPushToken], Error>) { lock.withLock { _result = result } }
    func pushTokens() async throws -> [RegisteredPushToken] { try lock.withLock { _reads += 1; return try _result.get() } }
}

@MainActor
@Suite("Push delivery store")
struct PushDeliveryStoreTests {
    private func listed(_ failed: String?) throws -> [RegisteredPushToken] {
        let failure = failed.map { #","lastFailure":{"at":"\#($0)","reason":"APNs answered 503"}"# } ?? ""
        let json = #"[{"token":"ab12cd34","environment":"production","label":"iPhone","registeredAt":"2026-11-20T09:00:00Z"\#(failure)}]"#
        return try JSONDecoder().decode([RegisteredPushToken].self, from: Data(json.utf8))
    }

    private final class Clock: @unchecked Sendable {
        private let lock = NSLock()
        private var _now = Date(timeIntervalSince1970: 1_795_000_000)
        var now: Date { lock.withLock { _now } }
        func advance(_ seconds: TimeInterval) { lock.withLock { _now = _now.addingTimeInterval(seconds) } }
    }

    private func store(_ source: ScriptedTokens, token: String? = "00ab12cd34", _ clock: Clock = Clock()) -> PushDeliveryStore {
        PushDeliveryStore(source: source, ownToken: { token }, now: { clock.now })
    }

    @Test("nothing is read, and nothing said, before Apple has given this phone a token")
    func noToken() async throws {
        let source = ScriptedTokens(.success(try listed("2026-11-26T09:00:00Z")))
        let store = store(source, token: nil)
        await store.refresh()
        #expect(source.reads == 0 && store.delivery == nil && store.lines == nil)
    }

    @Test("the phone's own token says whether it is failing, and a token that is not listed says nothing")
    func reads() async throws {
        let source = ScriptedTokens(.success(try listed("2026-11-26T09:00:00Z")))
        let store = store(source)
        #expect(store.lines == nil && !store.isFailing)
        await store.refresh()
        #expect(store.isFailing && store.lines?.count == 2)
        let other = PushDeliveryStore(source: source, ownToken: { "ffffffffffffffff" })
        await other.refresh()
        #expect(other.delivery == nil && other.lines == nil)
    }

    @Test("a read is kept for thirty seconds, a pull to reload asks anyway, and a screen that asks every few seconds does not read each time")
    func lifetime() async throws {
        let source = ScriptedTokens(.success(try listed(nil)))
        let clock = Clock()
        let store = store(source, clock)
        await store.refresh()
        clock.advance(PushDeliveryStore.lifetime - 1)
        await store.refresh()
        #expect(source.reads == 1)
        await store.refresh(force: true)
        #expect(source.reads == 2)
        clock.advance(PushDeliveryStore.lifetime + 1)
        await store.refresh()
        #expect(source.reads == 3)
    }

    @Test("what was known stays when the node cannot say: an older node, a read that failed, and the notice goes when a taking follows the refusal")
    func staysAndGoes() async throws {
        let source = ScriptedTokens(.success(try listed("2026-11-26T09:00:00Z")))
        let clock = Clock()
        let store = store(source, clock)
        await store.refresh()
        #expect(store.isFailing)
        source.set(.failure(NodeError.http(status: 404, message: nil)))
        await store.refresh(force: true)
        #expect(store.isFailing, "an older node has no such facts: nothing new is said, nothing is cleared")
        source.set(.failure(NodeError.http(status: 502, message: "down")))
        await store.refresh(force: true)
        #expect(store.isFailing)
        let json = #"[{"token":"ab12cd34","environment":"production","label":"iPhone","registeredAt":"2026-11-20T09:00:00Z","lastSucceededAt":"2026-11-26T10:00:00Z","lastFailure":{"at":"2026-11-26T09:00:00Z","reason":"x"}}]"#
        source.set(.success(try JSONDecoder().decode([RegisteredPushToken].self, from: Data(json.utf8))))
        await store.refresh(force: true)
        #expect(!store.isFailing && store.lines == nil)
    }
}
