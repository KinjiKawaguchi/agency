import AgencyProtocol
@testable import AgencyModel
import Foundation
import Testing

/// The shared cases: the web client reads the same file and has to make the same events.
@Suite("EventEngine")
struct EventEngineTests {
    private struct Case {
        let name: String
        let own: NotificationOwn
        let steps: [(reading: NotificationReading, expected: [NotificationEvent])]
    }

    private static func cases(from file: String = #filePath) throws -> [Case] {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        let root = try JSONSerialization.jsonObject(with: Data(contentsOf: url.appendingPathComponent("testdata/notifications.json"))) as! [String: Any]
        return (root["cases"] as! [[String: Any]]).map { item in
            let own = item["own"] as! [String: Any]
            return Case(
                name: item["name"] as! String,
                own: NotificationOwn(
                    deviceKey: own["deviceKey"] as! String, canApprove: own["canApprove"] as! Bool,
                    sentDecisionIds: Set(own["sentDecisionIds"] as! [String])
                ),
                steps: (item["steps"] as! [[String: Any]]).map { step in
                    (reading(step["reading"] as! [String: Any]), (step["expected"] as! [[String: String]]).map(event))
                }
            )
        }
    }

    private static func reading(_ json: [String: Any]) -> NotificationReading {
        NotificationReading(
            teams: (json["teams"] as! [[String: Any]]).map { team in
                NotificationReading.TeamReading(
                    org: team["org"] as! String, team: team["team"] as! String, ok: team["ok"] as! Bool,
                    mine: Set(team["mine"] as! [String]), waitingOnViewer: Set(team["waitingOnViewer"] as! [String]),
                    blocked: (team["blocked"] as! [[String: String]]).map { .init(ref: $0["ref"]!, reason: $0["reason"]!) }
                )
            },
            devices: json["devices"] as? [String],
            decisions: (json["decisions"] as? [[String: String]])?.map { .init(id: $0["id"]!, state: $0["state"]!) }
        )
    }

    private static func event(_ json: [String: String]) -> NotificationEvent {
        let kind = EventKind.allCases.first { $0.name == json["kind"] }!
        return NotificationEvent(kind: kind, key: json["key"]!)
    }

    @Test("every case of the shared file makes the events it lists, step by step")
    func sharedCases() throws {
        let cases = try Self.cases()
        #expect(cases.count >= 19)
        for item in cases {
            var engine = EventEngine()
            for (index, step) in item.steps.enumerated() {
                let events = engine.read(step.reading, own: item.own)
                #expect(events == step.expected, "\(item.name), step \(index + 1)")
            }
        }
    }

    @Test("the first step of every case is a baseline")
    func baselines() throws {
        for item in try Self.cases() {
            #expect(item.steps.first?.expected.isEmpty == true, "\(item.name)")
        }
    }
}
