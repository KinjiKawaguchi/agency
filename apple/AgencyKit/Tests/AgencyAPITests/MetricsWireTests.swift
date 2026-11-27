@testable import AgencyAPI
import Foundation
import Testing

@Suite("Metrics on the wire")
struct MetricsWireTests {
    private func read(_ json: String) throws -> Metrics { try JSONDecoder().decode(Metrics.self, from: Data(json.utf8)) }

    private func answer(series: [String] = [], extra: String = "") -> String {
        #"{"from":"2026-11-24T10:00:00Z","until":"2026-11-24T12:00:00Z","generatedAt":"2026-11-24T11:30:00Z","series":[\#(series.joined(separator: ","))]\#(extra)}"#
    }

    private func series(count: Int = 3, errors: Int = 0, buckets: [Int]? = nil, kind: String = "serve", extra: String = "") -> String {
        let counts = buckets ?? [count] + Array(repeating: 0, count: LatencySeries.bucketCount - 1)
        return #"{"kind":"\#(kind)","name":"GET /api/me","status":"2xx","group":"desk","isInNodeRead":true,"count":\#(count),"errors":\#(errors),"sumMs":3,"buckets":\#(counts),"byHour":[]\#(extra)}"#
    }

    @Test("the shared sample reads whole: its budget, its series and what is left out of one")
    func sample() throws {
        let metrics = try JSONDecoder().decode(Metrics.self, from: Fixtures.sample("metrics"))
        #expect(metrics.budget?.inNodeReadP95Ms == 100)
        #expect(metrics.series.count == 8 && metrics.objectives.map(\.group) == ["desk", "outcomes", "usage", "evaluation"])
        let read = try #require(metrics.series.first)
        #expect(read.kind == .serve && read.isInNodeRead && read.group == "desk" && read.p95Ms == 10)
        let runtime = try #require(metrics.series.first { $0.name == "runtime view" })
        #expect(runtime.p50Ms == 500 && runtime.p95Ms == nil, "a percentile beyond the last bucket is left out by the node")
        #expect(metrics.series.first { $0.peer != nil }?.peer == "node-b.example")
    }

    private func hourOf(count: Int = 4, buckets: String? = nil, p95: String = #","p95Ms":10"#) -> String {
        #"{"hour":"2026-11-24T11:00:00Z","count":\#(count),"errors":0\#(p95)\#(buckets.map { #","buckets":\#($0)"# } ?? "")}"#
    }

    private func hours(_ entry: String) -> String { series(extra: "").replacingOccurrences(of: #""byHour":[]"#, with: #""byHour":[\#(entry)]"#) }

    @Test("the hours of a series are read with their buckets when the node gave them, and without them as an older answer")
    func hourlyBuckets() throws {
        let counts = [0, 0, 4] + Array(repeating: 0, count: LatencySeries.bucketCount - 3)
        let given = try read(answer(series: [hours(hourOf(buckets: "\(counts)"))])).series[0].byHour
        #expect(given.count == 1 && given[0].count == 4 && given[0].p95Ms == 10 && given[0].buckets == counts)
        let without = try read(answer(series: [hours(hourOf())])).series[0].byHour
        #expect(without.count == 1 && without[0].buckets == nil)
        let older = try read(answer(series: [series().replacingOccurrences(of: #""byHour":[],"#, with: "").replacingOccurrences(of: #","byHour":[]"#, with: "")])).series[0]
        #expect(older.byHour.isEmpty, "a node that gives no hours")
        let beyond = try read(answer(series: [hours(hourOf(p95: ""))])).series[0].byHour
        #expect(beyond[0].p95Ms == nil)
    }

    @Test("an hour whose buckets deny its count, or that are not 15 non-negative numbers, refuses the whole answer")
    func hourlyBucketsRefused() {
        let fifteen = Array(repeating: 0, count: LatencySeries.bucketCount)
        let denying = [5] + Array(repeating: 0, count: LatencySeries.bucketCount - 1)
        let negative = [5, -1] + Array(repeating: 0, count: LatencySeries.bucketCount - 2)
        let short = [4] + Array(repeating: 0, count: LatencySeries.bucketCount - 2)
        for buckets in [fifteen, denying, negative, short, [4]] {
            #expect(throws: DecodingError.self) { try read(answer(series: [hours(hourOf(count: 4, buckets: "\(buckets)"))])) }
        }
        #expect(throws: DecodingError.self) { try read(answer(series: [hours(hourOf(count: -1))])) }
        #expect(throws: DecodingError.self) { try read(answer(series: [hours(hourOf(p95: #","p95Ms":-5"#))])) }
    }

    @Test("an answer with no series is a node that measured and has nothing yet, and no budget or objective is none")
    func empty() throws {
        let metrics = try read(answer())
        #expect(metrics.series.isEmpty && metrics.budget == nil && metrics.objectives.isEmpty)
    }

    @Test("objectives are read as the node states them, and a state the app does not know is unknown")
    func objectives() throws {
        let json = answer(extra: #","objectives":[{"group":"desk","p95Ms":50,"state":"met","measuredP95Ms":25},{"group":"usage","p95Ms":500,"state":"borrowed"},{"group":"teams","p95Ms":75,"state":"undetermined","measuredP95Ms":100}]"#)
        let objectives = try read(json).objectives
        #expect(objectives.map(\.state) == [.met, .unknown, .undetermined])
        #expect(objectives[0].measuredP95Ms == 25 && objectives[1].measuredP95Ms == nil)
    }

    @Test("a series whose buckets deny its count, whose bucket list is the wrong length, or that has more errors than calls refuses the whole answer")
    func denied() {
        let twoMissing = Array(repeating: 0, count: LatencySeries.bucketCount - 1)
        for bad in [
            series(count: 3, buckets: [2] + Array(repeating: 0, count: LatencySeries.bucketCount - 1)),
            series(count: 3, buckets: [3, 0, 0]),
            series(count: 3, errors: 4),
            series(count: 0, buckets: [0] + twoMissing + [0]).replacingOccurrences(of: #""sumMs":3"#, with: #""sumMs":-1"#),
            series(count: 3, extra: #","p95Ms":-5"#),
        ] {
            #expect(throws: DecodingError.self, Comment(rawValue: bad)) { try read(answer(series: [series(), bad])) }
        }
    }

    @Test("a kind of series the app does not know is read as unknown, and only a serve series can be an in-node read")
    func kinds() throws {
        let metrics = try read(answer(series: [series(kind: "stream"), series(kind: "call")]))
        #expect(metrics.series.map(\.kind) == [.unknown, .call])
        #expect(metrics.series.map(\.isInNodeRead) == [false, false], "the node says true for both here; a call is never one")
    }
}
