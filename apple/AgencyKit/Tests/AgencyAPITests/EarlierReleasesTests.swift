@testable import AgencyAPI
import Foundation
import Testing

/// What earlier nodes answered (`testdata/releases/<tag>/client/`) still decodes with the decoders of today. An app on a
/// phone is newer or older than its node, so a new app must read an older node's answers (`docs/compatibility.md`, "Checks in
/// CI"). Each file is decoded only as far as the app does: a field the app does not know is ignored, and a word of an enum
/// it does not know is that enum's `unknown`.
@Suite("Answers of earlier nodes")
struct EarlierReleasesTests {
    /// The answers the app never reads, by the file's name: the web client's alone (the activity feed, the browser's
    /// notice destinations, the push key) and the error body, which the client reads by hand.
    private static let notReadByTheApps: Set<String> = [
        "activity", "error", "push-subscription", "push-subscription-list", "push-vapid",
    ]

    private func decode<T: Decodable>(_ type: T.Type, _ data: Data) throws {
        _ = try JSONDecoder().decode(T.self, from: data)
    }

    /// Decodes `data` as the type the app reads the file `name` as, or returns false when no type is named for it.
    private func read(_ name: String, _ data: Data) throws -> Bool {
        switch name {
        case "decision-entry": try decode(DecisionEntry.self, data)
        case "decision-log", "decision-log-member": try decode(DecisionLog.self, data)
        case "decision-result", "decision-result-kept", "decision-result-queued": try decode(DecisionResult.self, data)
        case "device-change-result", "device-change-result-queued": try decode(DeviceChangeResult.self, data)
        case "device-list", "device-list-p256": try decode(DeviceList.self, data)
        case "device-request": try decode(DeviceRequest.self, data)
        case "evaluation": try decode(Evaluation.self, data)
        case "me", "me-funding", "me-hides-titles", "me-home-nodes", "me-member", "me-root-changes": try decode(Me.self, data)
        case "org": try decode(OrgDefinition.self, data)
        case "push-token": try decode(RegisteredPushToken.self, data)
        case "push-token-list": try decode(PushTokenList.self, data)
        case "session": try decode(Session.self, data)
        case "team-snapshot-local", "team-snapshot-replica": try decode(TeamSnapshot.self, data)
        case "usage": try decode(TokenUsage.self, data)
        default: return false
        }
        return true
    }

    @Test("the answers of v0.3.0 are kept")
    func kept() throws {
        let answers = try Fixtures.earlierAnswers()
        #expect(answers.contains { $0.tag == "v0.3.0" && $0.name == "me" })
    }

    @Test("every kept answer has a decoder, or is named as one the apps never read")
    func everyAnswerIsNamed() throws {
        let unnamed = try Fixtures.earlierAnswers().filter { !Self.notReadByTheApps.contains($0.name) && !isNamed($0.name) }
        #expect(unnamed.map { "\($0.tag)/\($0.name)" } == [], "name the file in `read`, with the type the app decodes it as")
    }

    @Test("every kept answer decodes")
    func everyAnswerDecodes() throws {
        var failures: [String] = []
        for answer in try Fixtures.earlierAnswers() where !Self.notReadByTheApps.contains(answer.name) {
            do {
                _ = try read(answer.name, answer.data)
            } catch {
                failures.append("\(answer.tag)/\(answer.name): \(error)")
            }
        }
        #expect(failures == [])
    }

    /// Whether `read` names a type for the file: with no bytes, a name it knows fails to decode (empty data is not JSON,
    /// whatever the type), and one it does not know returns false.
    private func isNamed(_ name: String) -> Bool {
        do { return try read(name, Data()) } catch { return true }
    }
}
