@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// Counts the attempts, and the waits a policy was asked to make.
private final class Recorder: @unchecked Sendable {
    private let lock = NSLock()
    private var _waits: [TimeInterval] = []
    private var _attempts = 0

    var waits: [TimeInterval] { lock.withLock { _waits } }
    var attempts: Int { lock.withLock { _attempts } }
    func wait(_ seconds: TimeInterval) { lock.withLock { _waits.append(seconds) } }
    func attempt() -> Int { lock.withLock { _attempts += 1; return _attempts } }

    func policy(_ delays: [TimeInterval] = [1, 2, 4, 8]) -> RetryPolicy {
        RetryPolicy(delays: delays, wait: { [self] in wait($0) })
    }
}

@Suite("RetryPolicy")
struct RetryPolicyTests {
    private let notReady = NodeError.http(status: 503, message: "the board has not been read yet; a refresh was started")

    @Test("a not-ready answer is asked again after each wait, and the answer that comes is the result")
    func recovers() async throws {
        let recorder = Recorder()
        let value = try await recorder.policy().run { () async throws -> String in
            if recorder.attempt() < 3 { throw notReady }
            return "ready"
        }
        #expect(value == "ready")
        #expect(recorder.attempts == 3)
        #expect(recorder.waits == [1, 2])
    }

    @Test("a read that keeps failing gives the last error once the waits are used up")
    func exhausted() async {
        let recorder = Recorder()
        await #expect(throws: NodeError.http(status: 503, message: "the board has not been read yet; a refresh was started")) {
            try await recorder.policy().run { () async throws -> Int in _ = recorder.attempt(); throw notReady }
        }
        #expect(recorder.attempts == 5)
        #expect(recorder.waits == [1, 2, 4, 8])
    }

    @Test("408, 429, 502, 503, 504 and a read with no answer at all are retryable; every other refusal and the ended sign-in are not")
    func whichErrors() {
        for status in [408, 429, 502, 503, 504] {
            #expect(RetryPolicy.isRetryable(NodeError.http(status: status, message: nil)), "\(status) is not ready yet")
        }
        for status in [400, 401, 403, 404, 409, 500] {
            #expect(!RetryPolicy.isRetryable(NodeError.http(status: status, message: nil)), "\(status) is final")
        }
        #expect(RetryPolicy.isRetryable(URLError(.timedOut)))
        #expect(RetryPolicy.isRetryable(URLError(.notConnectedToInternet)))
        #expect(!RetryPolicy.isRetryable(URLError(.cancelled)))
        #expect(!RetryPolicy.isRetryable(CancellationError()))
        #expect(!RetryPolicy.isRetryable(NodeError.sessionExpired))
        #expect(!RetryPolicy.isRetryable(NodeError.notSignedIn))
        #expect(!RetryPolicy.isRetryable(NodeError.unreadable("x")))
    }

    @Test("the standard policy waits 1, 2, 4, 8 and 15 seconds, five retries")
    func standard() {
        #expect(RetryPolicy.standard.delays == [1, 2, 4, 8, 15])
    }

    @Test("an answer that is not retryable is given at once, without a wait")
    func notRetried() async {
        let recorder = Recorder()
        await #expect(throws: NodeError.http(status: 404, message: nil)) {
            try await recorder.policy().run { () async throws -> Int in _ = recorder.attempt(); throw NodeError.http(status: 404, message: nil) }
        }
        #expect(recorder.attempts == 1)
        #expect(recorder.waits.isEmpty)
    }

    @Test("a cancelled task stops waiting instead of asking again")
    func cancelled() async {
        let recorder = Recorder()
        let policy = RetryPolicy(delays: [1, 2], wait: { _ in throw CancellationError() })
        await #expect(throws: CancellationError.self) {
            try await policy.run { () async throws -> Int in _ = recorder.attempt(); throw notReady }
        }
        #expect(recorder.attempts == 1)
    }
}
