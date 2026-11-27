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

    @Test("the shared sample, a slow node with long names, and the skeleton before the first answer and for another period, a node that does not measure and one that failed")
    func drawn() {
        let states: [(String, NodeSpeedAnswer?, Bool)] = [
            ("node-speed-sample", .ready(NodeSpeed(sample())), true), ("node-speed-slow", .ready(NodeSpeed(slow())), true),
            ("node-speed-sample-open", .ready(NodeSpeed(sample())), true), ("node-speed-slow-open", .ready(NodeSpeed(slow())), true),
            ("node-speed-first-read", nil, false), ("node-speed-other-period", nil, true),
            ("node-speed-unavailable", .unavailable, true), ("node-speed-could-not", .couldNotAnswer, true), ("node-speed-failed", .failed("HTTP 502"), true),
        ]
        for (name, answer, hasAnswered) in states {
            for scheme in [ColorScheme.light, .dark] {
                let content = NodeSpeedContent(days: 7, answer: answer, hasAnswered: hasAnswered, showsRoutes: name.hasSuffix("open"))
                let result = Snapshot.capture(name, content.padding(Theme.gutter), scheme: scheme)
                #expect(!result.png.isEmpty)
                #expect(result.size.width == Snapshot.phoneWidth, "\(name): \(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
            }
        }
    }

    // MARK: The hourly trend

    /// A day of hours: `calls` is the calls of each hour by bucket (an hour with none is left out, as the node may), and with `hasBuckets`
    /// false the hours carry only their own p95.
    private func daySeries(_ name: String, kind: String = "serve", group: String, isRead: Bool = false, hasBuckets: Bool = true, calls: [Int: [Int: Int]]) -> String {
        let start = Date(timeIntervalSince1970: 1_795_518_000 - 21 * 3600)
        func counts(_ spread: [Int: Int]) -> [Int] {
            var all = Array(repeating: 0, count: LatencySeries.bucketCount)
            for (bucket, count) in spread { all[bucket] = count }
            return all
        }
        let hours = calls.keys.sorted().map { hour -> String in
            let all = counts(calls[hour] ?? [:])
            let time = ISO8601DateFormatter().string(from: start.addingTimeInterval(Double(hour) * 3600))
            let p95 = Latency.percentile(of: all, percent: 95)
            let bound: String
            switch p95 {
            case .atMost(let ms)?: bound = #","p95Ms":\#(ms)"#
            case .beyond?, nil: bound = ""
            }
            return #"{"hour":"\#(time)","count":\#(all.reduce(0, +)),"errors":0\#(bound)\#(hasBuckets ? #","buckets":\#(all)"# : "")}"#
        }
        var total: [Int: Int] = [:]
        for spread in calls.values { for (bucket, count) in spread { total[bucket, default: 0] += count } }
        let all = counts(total)
        return #"{"kind":"\#(kind)","name":"\#(name)","status":"2xx","group":"\#(group)","isInNodeRead":\#(isRead),"count":\#(all.reduce(0, +)),"errors":0,"sumMs":0,"buckets":\#(all),"byHour":[\#(hours.joined(separator: ","))]}"#
    }

    private func day(_ series: [String], hours: Int = 24) -> Metrics {
        let json = #"{"from":"2026-11-23T14:00:00Z","until":"2026-11-24T14:00:00Z","generatedAt":"2026-11-24T14:00:00Z","budget":{"inNodeReadP95Ms":100},"series":[\#(series.joined(separator: ","))]}"#
        return try! JSONDecoder().decode(Metrics.self, from: Data(json.utf8))
    }

    @Test("the hourly trend: a line with gaps for hours with no call, the budget on the reads, a hollow mark for an hour slower than the last bound, and the approximate chart that says so")
    func trend() {
        var reads: [Int: [Int: Int]] = [:]
        var usage: [Int: [Int: Int]] = [:]
        for hour in 0..<24 where hour % 5 != 3 {
            reads[hour] = [2: 20, 3: 6 + hour % 4, hour > 18 ? 6 : 4: 3]
            usage[hour] = [8: 10, hour == 12 ? 14 : 9: hour == 12 ? 4 : 2]
        }
        let runtime = ["runtime view": [5: [14: 3], 6: [7: 2]]]
        let metrics = day([
            daySeries("GET /api/me", group: "desk", isRead: true, calls: reads),
            daySeries("GET /api/orgs/{org}/teams/{team}/usage", group: "usage", calls: usage),
            daySeries("runtime view", kind: "call", group: "runtime", calls: runtime["runtime view"] ?? [:]),
            daySeries(UIFixtures.unbrokenID, group: UIFixtures.unbrokenID, hasBuckets: false, calls: [20: [9: 7], 21: [11: 2]]),
        ])
        let speed = NodeSpeed(metrics)
        guard case .charts(let charts) = speed.trend else { Issue.record("should have charts"); return }
        #expect(charts.first?.budgetMs == 100 && charts.contains { !$0.isExact } && charts.contains { $0.isExact })
        let states: [(String, NodeSpeed.Trend, Bool)] = [("hour-trend", speed.trend, false), ("hour-trend-open", speed.trend, true), ("hour-trend-short", .onlyShort, false)]
        for (name, trend, opened) in states {
            for scheme in [ColorScheme.light, .dark] {
                let result = Snapshot.capture(name, HourTrendCard(trend: trend, showsTable: opened).padding(Theme.gutter), scheme: scheme)
                #expect(!result.png.isEmpty)
                #expect(result.size.width == Snapshot.phoneWidth, "\(name): \(result.size.width) points wide, the screen is \(Snapshot.phoneWidth)")
            }
        }
    }
}
#endif
