@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Hourly trend")
struct HourTrendTests {
    private let start = Date(timeIntervalSince1970: 1_795_518_000)  // 2026-11-24T11:00:00Z
    private func instant(_ text: String) -> Instant { try! JSONDecoder().decode(Instant.self, from: Data("\"\(text)\"".utf8)) }
    private func hours(_ count: Int) -> [Date] { (0..<count).map { start.addingTimeInterval(Double($0) * 3600) } }

    private func buckets(_ calls: [Int: Int]) -> [Int] {
        var counts = Array(repeating: 0, count: LatencySeries.bucketCount)
        for (index, count) in calls { counts[index] = count }
        return counts
    }

    /// One hour of a series: its JSON, and the calls it holds by bucket.
    private struct Hour {
        let json: String
        let calls: [Int: Int]
    }

    /// One hour of a series: `index` is its place from `start`; `buckets` is left out for a node that gives none.
    private func hour(_ index: Int, _ calls: [Int: Int], p95: Int? = nil, withBuckets: Bool = true) -> Hour {
        let counts = buckets(calls)
        let time = ISO8601DateFormatter().string(from: start.addingTimeInterval(Double(index) * 3600))
        let json = #"{"hour":"\#(time)","count":\#(counts.reduce(0, +)),"errors":0\#(p95.map { #","p95Ms":\#($0)"# } ?? "")\#(withBuckets ? #","buckets":\#(counts)"# : "")}"#
        return Hour(json: json, calls: calls)
    }

    /// A series whose totals are those of its hours, as the node's are.
    private func series(_ name: String, kind: String = "serve", group: String = "desk", isRead: Bool = false, _ hours: [Hour]) -> LatencySeries {
        var total: [Int: Int] = [:]
        for hour in hours { for (index, count) in hour.calls { total[index, default: 0] += count } }
        let counts = buckets(total)
        let json = #"{"kind":"\#(kind)","name":"\#(name)","status":"2xx","group":"\#(group)","isInNodeRead":\#(isRead),"count":\#(counts.reduce(0, +)),"errors":0,"sumMs":0,"buckets":\#(counts),"byHour":[\#(hours.map(\.json).joined(separator: ","))]}"#
        return try! JSONDecoder().decode(LatencySeries.self, from: Data(json.utf8))
    }

    // MARK: The hours of a range

    @Test("are each hour from the start to the end, oldest first, and none when the range is not hours of at most 7 days")
    func range() {
        #expect(HourTrend.hours(from: instant("2026-11-24T11:00:00Z"), until: instant("2026-11-24T14:00:00Z")) == hours(3))
        let from = instant("2026-11-24T11:00:00Z")
        #expect(HourTrend.hours(from: from, until: Instant.after(from, hours: HourTrend.maxHours)).count == HourTrend.maxHours)
        #expect(HourTrend.hours(from: from, until: Instant.after(from, hours: HourTrend.maxHours + 1)).isEmpty, "a range of more than 7 days comes with totals only")
        #expect(HourTrend.hours(from: from, until: from).isEmpty)
        #expect(HourTrend.hours(from: instant("2026-11-24T14:00:00Z"), until: from).isEmpty)
        #expect(HourTrend.hasHours(days: 1) && HourTrend.hasHours(days: 7) && !HourTrend.hasHours(days: 30))
    }

    // MARK: The points

    @Test("an hour with no call is a gap, never a 0, and so is an entry that has no call")
    func gaps() {
        let points = HourTrend.points(of: [series("a", [hour(0, [2: 5]), hour(2, [6: 3]), hour(3, [:])])], hours: hours(4))
        #expect(points.map(\.bound) == [.atMost(5), nil, .atMost(100), nil])
        #expect(points.map(\.count) == [5, 0, 3, 0])
    }

    @Test("with the buckets of every route the point is the group's own p95, read from the buckets added, not the slowest route's")
    func exact() {
        // 95 calls within 1 ms and 5 within 30 s: the group's p95 is the 95th of 100 (1 ms), while the slowest route's own p95 is 30 s.
        let fast = series("fast", [hour(0, [0: 95], p95: 1)])
        let slow = series("slow", [hour(0, [13: 5], p95: 30_000)])
        let trend = HourTrend.chartPoints(of: [fast, slow], hours: hours(1))
        #expect(trend.isExact && trend.points[0].bound == .atMost(1) && trend.points[0].count == 100)
        // The same without buckets: only the slowest route's p95 is known, and the group's is at most that.
        let without = HourTrend.chartPoints(of: [series("fast", [hour(0, [0: 95], p95: 1, withBuckets: false)]), series("slow", [hour(0, [13: 5], p95: 30_000, withBuckets: false)])], hours: hours(1))
        #expect(!without.isExact && without.points[0].bound == .atMost(30_000))
    }

    @Test("one route without buckets makes the whole chart approximate: a line never mixes two meanings")
    func mixed() {
        let withBuckets = series("a", [hour(0, [0: 95], p95: 1), hour(1, [0: 10], p95: 1)])
        let without = series("b", [hour(0, [13: 5], p95: 30_000, withBuckets: false)])
        let trend = HourTrend.chartPoints(of: [withBuckets, without], hours: hours(2))
        #expect(!trend.isExact)
        #expect(trend.points.map(\.bound) == [.atMost(30_000), .atMost(1)], "every hour is the slowest route's p95, also the hour that had buckets")
        #expect(trend.points.allSatisfy { !$0.isExact || $0.bound == nil })
    }

    @Test("whichever order the routes come in, a route without buckets makes the chart approximate, and an entry with no call says nothing of the hour")
    func order() {
        let without = series("b", [hour(0, [13: 5], p95: 30_000, withBuckets: false)])
        let withBuckets = series("a", [hour(0, [0: 95], p95: 1)])
        let trend = HourTrend.chartPoints(of: [without, withBuckets], hours: hours(1))
        #expect(!trend.isExact && trend.points[0].bound == .atMost(30_000))
        // An entry of the hour that holds no call has no p95 and must not make the hour beyond.
        let empty = series("c", [hour(0, [:], withBuckets: false)])
        #expect(HourTrend.points(of: [empty, withBuckets], hours: hours(1), useBuckets: false)[0].bound == .atMost(1))
    }

    @Test("a route with calls and no p95 is beyond the last bound, and makes the hour beyond when another is faster")
    func beyond() {
        let none = series("slowest", [hour(0, [14: 3], withBuckets: false)])
        let fast = series("fast", [hour(0, [2: 10], p95: 5, withBuckets: false)])
        #expect(HourTrend.points(of: [fast, none], hours: hours(1), useBuckets: false)[0].bound == .beyond)
        #expect(HourTrend.points(of: [none, fast], hours: hours(1), useBuckets: false)[0].bound == .beyond, "whatever the order")
        #expect(HourTrend.points(of: [series("a", [hour(0, [14: 3])])], hours: hours(1))[0].bound == .beyond, "from buckets, too")
        #expect(HourScale.share(of: .beyond) == 1 && HourScale.share(of: nil) == nil)
    }

    @Test("buckets are not trusted from an hour that sums wrongly: the whole answer is refused on the wire, so none reaches here")
    func nearestRank() {
        // 19 within 1 ms and 1 within 2 ms: the 95th of 20 is the 19th, 1 ms; one more in the slower bucket moves it to 2 ms.
        #expect(HourTrend.points(of: [series("a", [hour(0, [0: 19, 1: 1])])], hours: hours(1))[0].bound == .atMost(1))
        #expect(HourTrend.points(of: [series("a", [hour(0, [0: 18, 1: 2])])], hours: hours(1))[0].bound == .atMost(2))
    }

    // MARK: The charts

    @Test("the in-node reads come first with the budget, then each route group, then each group of calls, busiest first, and a chart with no calls is not drawn")
    func charts() {
        let reads = series("GET /api/me", group: "desk", isRead: true, [hour(0, [2: 4])])
        let usage = series("GET /api/orgs/{org}/teams/{team}/usage", group: "usage", [hour(0, [9: 30])])
        let peer = series("peer get_view", kind: "call", group: "peer", [hour(0, [6: 2])])
        let silent = series("GET /api/devices", group: "devices", [hour(5, [2: 1])])  // an hour outside the range
        let charts = HourTrend.charts(of: [reads, usage, peer, silent], budgetMs: 100, hours: hours(2))
        #expect(charts.map(\.id) == ["in-node-reads", "serve-usage", "serve-desk", "call-peer"])
        #expect(charts.map(\.budgetMs) == [100, nil, nil, nil], "the budget is drawn only on the chart of the reads it applies to")
        #expect(charts[0].title == "ノード内の読み取り" && charts[3].title == "呼び出し: 別のノード")
        #expect(HourTrend.charts(of: [reads], budgetMs: nil, hours: hours(2)).map(\.id) == ["serve-desk"], "no budget, no chart of the reads")
    }

    @Test("what a chart says to anyone who cannot see it: a sentence, and the hours with calls, newest first")
    func words() {
        let chart = HourTrend.charts(of: [series("a", group: "usage", [hour(0, [2: 4]), hour(2, [9: 3])])], budgetMs: nil, hours: hours(3))[0]
        #expect(chart.summary == "使用量: ルートをまとめた各時間の p95。呼び出しのあった時間は 2 時間で、直近は 1 秒 以下。")
        #expect(chart.rows.map(\.calls) == ["3", "4"] && chart.rows.map(\.p95) == ["1 秒 以下", "5 ミリ秒 以下"])
        #expect(chart.rows.map(\.hour) == [JaFormat.dateTime(start.addingTimeInterval(7200)), JaFormat.dateTime(start)], "newest first, and only the hours with calls")
        let approximate = HourTrend.charts(of: [series("a", group: "usage", [hour(0, [2: 4], p95: 5, withBuckets: false)])], budgetMs: nil, hours: hours(1))[0]
        #expect(!approximate.isExact && approximate.summary == "使用量: 各時間でいちばん遅いルートの p95。呼び出しのあった時間は 1 時間で、直近は 5 ミリ秒 以下。")
    }

    // MARK: The speed

    @Test("a range of more than 7 days says it has no hours; hours with no call draw nothing")
    func trendOfSpeed() throws {
        func metrics(until: String, series: [LatencySeries]) -> Metrics {
            Metrics(range: (instant("2026-11-24T11:00:00Z"), instant(until)), generatedAt: instant(until), budget: nil, objectives: [], series: series)
        }
        let one = series("a", [hour(0, [2: 4])])
        #expect(NodeSpeed(metrics(until: "2026-12-24T11:00:00Z", series: [one])).trend == .onlyShort)
        guard case .charts(let drawn) = NodeSpeed(metrics(until: "2026-11-24T14:00:00Z", series: [one])).trend else { Issue.record("should have charts"); return }
        #expect(drawn.count == 1)
        #expect(NodeSpeed(metrics(until: "2026-11-24T14:00:00Z", series: [series("a", [hour(0, [:])])])).trend == .charts([]))
    }

    @Test("the shared sample: its reads in the hour they were made, with no call in the hour before")
    func sample() throws {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let metrics = try JSONDecoder().decode(Metrics.self, from: Data(contentsOf: url.appendingPathComponent("testdata/metrics.sample.json")))
        guard case .charts(let charts) = NodeSpeed(metrics).trend else { Issue.record("should have charts"); return }
        let reads = try #require(charts.first { $0.id == "in-node-reads" })
        #expect(reads.isExact && reads.points.map(\.bound) == [nil, .atMost(50)] && reads.points.map(\.count) == [0, 24])
        let usage = try #require(charts.first { $0.id == "serve-usage" })
        #expect(usage.points.map(\.bound) == [nil, .atMost(30_000)] && usage.points[1].count == 11)
        let runtime = try #require(charts.first { $0.id == "call-runtime" })
        #expect(runtime.points.map(\.bound) == [nil, .beyond], "a route with calls and no p95 is beyond the last bound")
        let peer = try #require(charts.first { $0.id == "call-peer" })
        #expect(peer.points.map(\.bound) == [.atMost(100), nil])
    }
}

private extension Instant {
    static func after(_ instant: Instant, hours: Int) -> Instant {
        let text = ISO8601DateFormatter().string(from: instant.date.addingTimeInterval(Double(hours) * 3600))
        return try! JSONDecoder().decode(Instant.self, from: Data("\"\(text)\"".utf8))
    }
}
