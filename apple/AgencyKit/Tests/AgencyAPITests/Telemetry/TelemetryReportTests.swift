import Foundation
import Testing
@testable import AgencyAPI

@Suite("The report")
struct TelemetryReportTests {
    private func sample(_ route: String = "GET /api/me", ms: Int = 12) -> CallSample { CallSample(route: route, status: "2xx", ms: ms) }

    @Test("a report is the client's name and the samples, in canonical JSON")
    func body() throws {
        let report = TelemetryReport(samples: [sample(), CallSample(route: "GET /api/orgs/{org}/effort", status: CallSample.network, ms: 31_000)])
        let text = String(decoding: try report.body, as: UTF8.self)
        #expect(text == #"{"client":"ios","samples":[{"ms":12,"route":"GET /api/me","status":"2xx"},{"ms":31000,"route":"GET /api/orgs/{org}/effort","status":"network"}]}"#)
    }

    @Test("a report holds no more than 200 samples")
    func count() {
        let many = Array(repeating: sample(), count: 450)
        #expect(TelemetryReport.fitting(many).samples.count == 200)
        #expect(TelemetryReport.fitting(Array(many.prefix(199))).samples.count == 199)
        #expect(TelemetryReport.fitting([]).samples.isEmpty)
    }

    @Test("a report holds no more than 16 KiB, taking from the first sample, and one more sample would not have fitted")
    func bytes() throws {
        let long = sample("POST /api/orgs/{org}/teams/{team}/envelopes", ms: 30_001)
        let report = TelemetryReport.fitting(Array(repeating: long, count: 200))
        let size = try report.body.count
        #expect(size <= TelemetryReport.maxBytes)
        #expect(report.samples.count < 200, "200 of these are more than 16 KiB")
        let one = try TelemetryReport(samples: Array(repeating: long, count: report.samples.count + 1)).body.count
        #expect(one > TelemetryReport.maxBytes)
        #expect(report.samples.allSatisfy { $0 == long })
    }

    @Test("the samples are taken in order, from the oldest")
    func order() {
        let ordered = (1...5).map { sample(ms: $0) }
        #expect(TelemetryReport.fitting(ordered).samples == ordered)
    }

    @Test("the report is posted to /api/telemetry with the write token, and the answer is read")
    func posted() async throws {
        let transport = ScriptedTransport([
            ("GET /api/session", json(#"{"writeToken":"w-1","now":"2026-11-24T12:00:00Z"}"#)),
            ("POST /api/telemetry", json(#"{"accepted":2,"dropped":0}"#)),
        ])
        let client = NodeClient(transport: transport, accessToken: { "access-jwt" })
        let answer = try await client.reportTelemetry(TelemetryReport(samples: [sample(), sample("GET /api/me", ms: 3)]))
        #expect(answer == TelemetryAnswer(accepted: 2, dropped: 0))
        let post = try #require(transport.requests.last)
        #expect(post.method == "POST" && post.path == "/api/telemetry")
        #expect(post.headers["x-agency-token"] == "w-1")
        #expect(String(decoding: post.body ?? Data(), as: UTF8.self).hasPrefix(#"{"client":"ios","samples":["#))
    }

    @Test("a refusal is an error the caller can read the status of")
    func refused() async {
        let transport = ScriptedTransport([
            ("GET /api/session", json(#"{"writeToken":"w-1","now":"2026-11-24T12:00:00Z"}"#)),
            ("POST /api/telemetry", json(#"{"error":"owner only"}"#, status: 403)),
        ])
        let client = NodeClient(transport: transport, accessToken: { "access-jwt" })
        await #expect(throws: NodeError.self) { try await client.reportTelemetry(TelemetryReport(samples: [sample()])) }
    }
}
