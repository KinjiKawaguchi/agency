@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Node speed")
struct NodeSpeedTests {
    private func instant(_ text: String) -> Instant { try! JSONDecoder().decode(Instant.self, from: Data("\"\(text)\"".utf8)) }

    private func metrics(series: [LatencySeries], budget: Int? = 100, objectives: [Objective] = []) -> Metrics {
        Metrics(
            range: (instant("2026-11-24T10:00:00Z"), instant("2026-11-24T12:00:00Z")), generatedAt: instant("2026-11-24T11:30:00Z"),
            budget: budget.map(Budget.init(inNodeReadP95Ms:)), objectives: objectives, series: series
        )
    }

    private func sample() throws -> Metrics {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try JSONDecoder().decode(Metrics.self, from: Data(contentsOf: url.appendingPathComponent("testdata/metrics.sample.json")))
    }

    /// `count` calls, each in the bucket of `bound` milliseconds (`nil`: slower than the last).
    private func series(_ name: String, kind: LatencySeries.Kind = .serve, group: String = "desk", peer: String? = nil, isRead: Bool = false, _ calls: [(bound: Int?, count: Int)], errors: Int = 0) -> LatencySeries {
        var buckets = Array(repeating: 0, count: LatencySeries.bucketCount)
        for call in calls { buckets[call.bound.flatMap(LatencySeries.bucketBoundsMs.firstIndex(of:)) ?? LatencySeries.bucketCount - 1] += call.count }
        return LatencySeries(kind: kind, name: name, group: group, peer: peer, isInNodeRead: isRead, count: buckets.reduce(0, +), errors: errors, buckets: buckets)
    }

    // MARK: The percentile

    @Test("the percentile is the upper bound of the bucket of the nearest rank, and beyond the last bucket has no bound")
    func percentile() {
        func buckets(_ at: [Int: Int]) -> [Int] { (0..<LatencySeries.bucketCount).map { at[$0] ?? 0 } }
        #expect(Latency.percentile(of: buckets([:]), percent: 95) == nil, "no duration is not 0 ms")
        #expect(Latency.percentile(of: buckets([0: 1]), percent: 99) == .atMost(1))
        // 20 calls: 19 in the 5th bucket (25 ms), 1 in the 10th (1000 ms): the 95th is the 19th, the 99th the 20th.
        #expect(Latency.percentile(of: buckets([4: 19, 9: 1]), percent: 95) == .atMost(25))
        #expect(Latency.percentile(of: buckets([4: 19, 9: 1]), percent: 99) == .atMost(1000))
        #expect(Latency.percentile(of: buckets([4: 18, 9: 2]), percent: 95) == .atMost(1000), "the 19th of 20 is in the slower bucket")
        #expect(Latency.percentile(of: buckets([4: 3, 14: 1]), percent: 99) == .beyond)
        #expect(Latency.percentile(of: buckets([4: 3, 14: 1]), percent: 50) == .atMost(25))
        #expect(Latency.percentile(of: buckets([1: 7]), percent: 50) == .atMost(2), "one bucket is every percentile")
    }

    @Test("every percentile of the shared sample's series is the node's own, and one the node leaves out is beyond the last bucket")
    func theNodesOwn() throws {
        let sample = try sample()
        for one in sample.series {
            for (percent, given) in [(50, one.p50Ms), (95, one.p95Ms), (99, one.p99Ms)] {
                let ours = Latency.percentile(of: one.buckets, percent: percent)
                #expect(ours == given.map(Bound.atMost) ?? (one.count > 0 ? Bound.beyond : nil), Comment(rawValue: "\(one.name) p\(percent)"))
            }
        }
    }

    @Test("a group's percentile is read from its series' buckets put together, never an average of their percentiles")
    func together() {
        // Two series of one group: 10 calls at 1 ms and 10 calls at 1000 ms. Their own p95 are 1 and 1000: the average, 500, is no bound.
        let groups = Latency.groups(of: [series("GET /a", [(1, 10)]), series("GET /b", [(1000, 10)])], kind: .serve)
        let group = groups[0].row
        #expect(group.count == 20 && group.p50 == .atMost(1) && group.p95 == .atMost(1000) && group.p99 == .atMost(1000))
    }

    // MARK: Rows

    @Test("rows are by group, busiest first and by name when equal, each group with its routes; a series with no call is no row")
    func rows() {
        let all = [
            series("GET /a", group: "desk", [(5, 3)]), series("GET /b", group: "desk", [(5, 9)]), series("GET /c", group: "usage", [(5, 12)]),
            series("GET /d", group: "teams", [(5, 14)]), series("GET /none", group: "teams", []), series("runtime view", kind: .call, group: "runtime", [(5, 4)]),
        ]
        let groups = Latency.groups(of: all, kind: .serve)
        #expect(groups.map(\.row.key) == ["teams", "desk", "usage"], "teams 14 first, then desk and usage 12 and 12 by name")
        #expect(groups[1].routes.map(\.key) == ["GET /b", "GET /a"])
        #expect(groups[0].routes.map(\.key) == ["GET /d"])
        #expect(Latency.groups(of: all, kind: .call).map(\.row.key) == ["runtime"])
        #expect(Latency.groups(of: all, kind: .unknown).isEmpty)
    }

    @Test("statuses of one route are one row, and calls to different peers are different rows")
    func routes() {
        let all = [
            series("GET /a", [(5, 3)]), series("GET /a", [(10, 2)], errors: 1),
            series("peer get_view", kind: .call, group: "peer", peer: "node-b.example", [(50, 2)]), series("peer get_view", kind: .call, group: "peer", peer: "node-c.example", [(50, 5)]),
        ]
        #expect(Latency.groups(of: all, kind: .serve)[0].routes.map { [$0.key, "\($0.count)", "\($0.errors)"] } == [["GET /a", "5", "1"]])
        #expect(Latency.groups(of: all, kind: .call)[0].routes.map(\.key) == ["peer get_view (node-c.example)", "peer get_view (node-b.example)"])
    }

    // MARK: The budget

    @Test("a p95 is known only as its bucket: within when the bucket's top is at most the budget, over when its bottom is at least the budget, otherwise undetermined")
    func verdict() {
        // The budget of 100 ms is a bucket edge: it is never undetermined.
        #expect(Latency.verdict(of: .atMost(100), budgetMs: 100) == .within)
        #expect(Latency.verdict(of: .atMost(250), budgetMs: 100) == .over)
        #expect(Latency.verdict(of: .atMost(50), budgetMs: 100) == .within)
        #expect(Latency.verdict(of: .atMost(1), budgetMs: 100) == .within)
        #expect(Latency.verdict(of: .beyond, budgetMs: 100) == .over)
        #expect(Latency.verdict(of: nil, budgetMs: 100) == nil)
        // A budget inside a bucket cannot be told: 75 ms against (50, 100].
        #expect(Latency.verdict(of: .atMost(100), budgetMs: 75) == .undetermined)
        #expect(Latency.verdict(of: .atMost(50), budgetMs: 75) == .within)
        #expect(Latency.verdict(of: .atMost(250), budgetMs: 75) == .over)
        #expect(Latency.verdict(of: .atMost(250), budgetMs: 175) == .undetermined)
        #expect(Latency.verdict(of: .beyond, budgetMs: 30_000) == .over)
        #expect(Latency.verdict(of: .beyond, budgetMs: 40_000) == .undetermined)
        #expect(Latency.verdict(of: .atMost(1), budgetMs: 0) == .over, "the first bucket starts at 0")
    }

    @Test("a verdict that cannot be told is said so, with the budget in words, and its reads are not listed as breaking it")
    func undetermined() throws {
        let all = [series("GET /api/me", isRead: true, [(100, 20)])]
        let reading = try #require(NodeSpeed(metrics(series: all, budget: 75)).budget)
        #expect(reading.verdict == .undetermined && reading.overRoutes.isEmpty)
        #expect(reading.verdictLine == "上限に対して判断できません: p95 は 100 ミリ秒 以下 で、上限の 75 ミリ秒 はその範囲の中にあります。")
        #expect(reading.requestsLine == "20 件の読み取りを計測しました。")
    }

    @Test("only in-node reads count for the budget, put together, and the routes that break it are listed")
    func budget() throws {
        let all = [
            series("GET /api/me", isRead: true, [(5, 18)]), series("GET /api/slow", isRead: true, [(250, 3)]),
            series("GET /api/usage", group: "usage", [(5000, 50)]), series("GET /api/slow", kind: .call, group: "peer", isRead: true, [(5000, 50)]),
        ]
        let reading = try #require(NodeSpeed(metrics(series: all)).budget)
        #expect(reading.reads?.count == 21 && reading.reads?.p95 == .atMost(250) && reading.verdict == .over)
        #expect(reading.overRoutes.map(\.key) == ["GET /api/slow"])
        #expect(reading.verdictLine == "上限を超えています: p95 は 250 ミリ秒 以下。" && reading.requestsLine == "21 件の読み取りを計測しました。")
        #expect(reading.lead.hasSuffix("p95 が 100 ミリ秒 以下です。"))
        let within = try #require(NodeSpeed(metrics(series: [series("GET /api/me", isRead: true, [(50, 10)])])).budget)
        #expect(within.verdictLine == "上限の内です: p95 は 50 ミリ秒 以下。" && within.overRoutes.isEmpty)
    }

    @Test("with no in-node read measured the budget says so, and with no budget named there is no budget card")
    func noReads() throws {
        let none = try #require(NodeSpeed(metrics(series: [series("GET /api/usage", group: "usage", [(5000, 5)])])).budget)
        #expect(none.reads == nil && none.verdict == nil && none.requestsLine == nil)
        #expect(none.verdictLine == "この期間は、ノード内の読み取りが計測されていません。")
        #expect(NodeSpeed(metrics(series: [series("GET /api/me", isRead: true, [(5, 1)])], budget: nil)).budget == nil)
    }

    // MARK: The words

    @Test("every bound is written at most, in milliseconds or seconds, and beyond the last bucket is slower than 30 seconds")
    func words() {
        #expect(Ja.NodeSpeed.bound(.atMost(1)) == "1 ミリ秒 以下")
        #expect(Ja.NodeSpeed.bound(.atMost(250)) == "250 ミリ秒 以下")
        #expect(Ja.NodeSpeed.bound(.atMost(1000)) == "1 秒 以下")
        #expect(Ja.NodeSpeed.bound(.atMost(2500)) == "2.5 秒 以下")
        #expect(Ja.NodeSpeed.bound(.atMost(30_000)) == "30 秒 以下")
        #expect(Ja.NodeSpeed.bound(.beyond) == "30 秒 より遅い")
        #expect(Ja.NodeSpeed.days(1) == "直近 1 日" && Ja.NodeSpeed.days(7) == "直近 7 日")
        #expect(Ja.NodeSpeed.Groups.name("desk") == "デスク" && Ja.NodeSpeed.Groups.name("newgroup") == "newgroup")
    }

    @Test("the objectives are said as the node states them, with its state and what it measured, and the app compares nothing")
    func objectives() throws {
        let json = #"[{"group":"desk","p95Ms":50,"state":"notMet","measuredP95Ms":100},{"group":"usage","p95Ms":2500,"state":"met"},{"group":"other","p95Ms":10,"state":"unknown"},{"group":"teams","p95Ms":75,"state":"undetermined","measuredP95Ms":100}]"#
        let lines = NodeSpeed(metrics(series: [series("GET /a", [(5, 1)])], objectives: try JSONDecoder().decode([Objective].self, from: Data(json.utf8)))).objectives
        #expect(lines.map(\.target) == ["デスク: p95 50 ミリ秒 以下", "使用量: p95 2.5 秒 以下", "その他: p95 10 ミリ秒 以下", "チーム: p95 75 ミリ秒 以下"])
        #expect(lines.map(\.stateWord) == ["未達", "達成", "まだ判断できません", "この目標に対して判断できません"], "undetermined (the objective is inside the bucket) is not unknown (no call)")
        #expect(lines.map(\.measured) == ["計測値: 100 ミリ秒 以下。", nil, nil, "計測値: 100 ミリ秒 以下。"])
    }

    @Test("an answer with no series says nothing was measured, which is not 0 ms, and has no rows")
    func nothingMeasured() throws {
        let speed = NodeSpeed(metrics(series: []))
        #expect(speed.hasNoSeries && speed.routeGroups.isEmpty && speed.callGroups.isEmpty)
        #expect(Ja.NodeSpeed.none == "この期間は、まだ何も計測されていません。")
    }
}
