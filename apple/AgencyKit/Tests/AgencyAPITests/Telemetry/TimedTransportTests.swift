import Foundation
import Testing
@testable import AgencyAPI

/// What a timed transport told, in the order it was told.
private final class Seen: @unchecked Sendable {
    private let lock = NSLock()
    private var samples: [CallSample] = []
    func add(_ sample: CallSample) { lock.withLock { samples.append(sample) } }
    var all: [CallSample] { lock.withLock { samples } }
}

/// Answers with a status or fails with an error, after a wait.
private struct Slow: HTTPTransport {
    var status = 200
    var error: (any Error)?
    var wait: Duration = .zero

    func send(_ request: HTTPRequest) async throws -> HTTPResponse {
        try await Task.sleep(for: wait)
        if let error { throw error }
        return HTTPResponse(status: status, body: Data())
    }
}

@Suite("Timing the calls")
struct TimedTransportTests {
    private func timed(_ base: Slow) -> (TimedTransport, Seen) {
        let seen = Seen()
        return (TimedTransport(base) { seen.add($0) }, seen)
    }

    private let view = HTTPRequest(method: "GET", path: "/api/orgs/acme/teams/app/view?secret=1", headers: ["cf-access-token": "t"], body: Data("body".utf8))

    @Test("a call is named by its route and the class of its status, and nothing of the path, query, header or body is in it")
    func sample() async throws {
        for (status, expected) in [(200, "2xx"), (204, "2xx"), (302, "3xx"), (404, "4xx"), (429, "4xx"), (500, "5xx"), (503, "5xx")] {
            let (transport, seen) = timed(Slow(status: status))
            let response = try await transport.send(view)
            #expect(response.status == status)
            #expect(seen.all.map(\.route) == ["GET /api/orgs/{org}/teams/{team}/view"], Comment(rawValue: "\(status)"))
            #expect(seen.all.map(\.status) == [expected], Comment(rawValue: "\(status)"))
        }
    }

    @Test("the time is the whole milliseconds the call took")
    func milliseconds() async throws {
        let (transport, seen) = timed(Slow(wait: .milliseconds(120)))
        _ = try await transport.send(view)
        let ms = try #require(seen.all.first?.ms)
        #expect(ms >= 100 && ms < 3_000, Comment(rawValue: "\(ms)"))
        let (longer, longerSeen) = timed(Slow(wait: .milliseconds(1_250)))
        _ = try await longer.send(view)
        let seconds = try #require(longerSeen.all.first?.ms)
        #expect(seconds >= 1_250 && seconds < 4_000, "whole seconds count too: \(seconds)")
    }

    @Test("a call that got no answer is a network failure, and its error goes on to the caller")
    func network() async {
        let (transport, seen) = timed(Slow(error: URLError(.timedOut)))
        await #expect(throws: URLError.self) { try await transport.send(view) }
        #expect(seen.all.map(\.status) == [CallSample.network])
        #expect(seen.all.map(\.route) == ["GET /api/orgs/{org}/teams/{team}/view"])
    }

    @Test("a call the caller cancelled is not reported")
    func cancelled() async {
        for error in [URLError(.cancelled) as any Error, CancellationError()] {
            let (transport, seen) = timed(Slow(error: error))
            await #expect(throws: (any Error).self) { try await transport.send(view) }
            #expect(seen.all.isEmpty)
        }
        let (transport, seen) = timed(Slow(wait: .seconds(30)))
        let call = Task { try await transport.send(view) }
        call.cancel()
        _ = try? await call.value
        #expect(seen.all.isEmpty)
    }

    @Test("a call to a route that is not listed, or with a status the node would drop, is not reported")
    func notReported() async throws {
        let (transport, seen) = timed(Slow())
        _ = try await transport.send(HTTPRequest(method: "POST", path: "/api/telemetry"))
        _ = try await transport.send(HTTPRequest(method: "POST", path: "/app/redeem"))
        _ = try await transport.send(HTTPRequest(method: "GET", path: "/api/orgs/acme/unlisted"))
        #expect(seen.all.isEmpty)
        for status in [100, 199, 600, 0] {
            let (other, otherSeen) = timed(Slow(status: status))
            _ = try await other.send(view)
            #expect(otherSeen.all.isEmpty, Comment(rawValue: "\(status)"))
        }
    }
}
