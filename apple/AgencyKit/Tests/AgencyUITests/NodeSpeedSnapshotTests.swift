#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// The node's response times drawn at phone width. The rule checked is the owner's: nothing is wider than the screen, even with
/// long route names, an unbroken group name and many groups. With `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Node speed renderings", .serialized)
struct NodeSpeedSnapshotTests {
    private func sample() -> Metrics {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try! JSONDecoder().decode(Metrics.self, from: Data(contentsOf: url.appendingPathComponent("testdata/metrics.sample.json")))
    }

    private func instant(_ text: String) -> Instant { try! JSONDecoder().decode(Instant.self, from: Data("\"\(text)\"".utf8)) }

    private func slow() -> Metrics {
        var slowBuckets = Array(repeating: 0, count: LatencySeries.bucketCount)
        slowBuckets[8] = 30
        slowBuckets[14] = 2
        let long = "GET /api/orgs/{org}/teams/{team}/\(UIFixtures.unbrokenID)"
        let series = [
            LatencySeries(kind: .serve, name: long, group: UIFixtures.unbrokenID, isInNodeRead: true, count: 32, errors: 1, buckets: slowBuckets),
            LatencySeries(kind: .call, name: "peer get_view", group: "peer", peer: "node-b.example", count: 32, buckets: slowBuckets),
        ]
        let objectives = try! JSONDecoder().decode([Objective].self, from: Data(#"[{"group":"desk","p95Ms":50,"state":"notMet","measuredP95Ms":500},{"group":"usage","p95Ms":2500,"state":"unknown"}]"#.utf8))
        return Metrics(range: (instant("2026-11-24T10:00:00Z"), instant("2026-11-24T12:00:00Z")), generatedAt: instant("2026-11-24T11:30:00Z"), budget: Budget(inNodeReadP95Ms: 100), objectives: objectives, series: series)
    }

    @Test("the shared sample, a slow node with long names, and the lines for nothing measured, a node that does not measure and one that failed")
    func drawn() {
        let states: [(String, NodeSpeedAnswer?)] = [
            ("node-speed-sample", .ready(NodeSpeed(sample()))), ("node-speed-slow", .ready(NodeSpeed(slow()))),
            ("node-speed-loading", nil), ("node-speed-unavailable", .unavailable), ("node-speed-failed", .failed("HTTP 502")),
        ]
        for (name, answer) in states {
            for scheme in [ColorScheme.light, .dark] {
                let result = Snapshot.capture(name, NodeSpeedContent(days: 7, answer: answer).padding(Theme.gutter), scheme: scheme)
                #expect(!result.png.isEmpty)
                #expect(result.size.width == Snapshot.phoneWidth, "\(name): \(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
            }
        }
    }
}
#endif
