@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

private final class FakePoster: PushTokenPoster, @unchecked Sendable {
    private let lock = NSLock()
    private var answers: [Result<Void, Error>]
    private(set) var calls: [[String]?] = []
    private(set) var devices: [String?] = []

    init(_ answers: [Result<Void, Error>]) { self.answers = answers }

    func registerPushToken(
        token: String, environment: String, label: String, events: [String]?, device: String?
    ) async throws -> RegisteredPushToken {
        try lock.withLock {
            calls.append(events)
            devices.append(device)
            try answers.removeFirst().get()
        }
        let answer = #"{"environment":"sandbox","label":"iPhone","registeredAt":"2026-11-24T12:00:00Z","token":"ab12"}"#
        return try JSONDecoder().decode(RegisteredPushToken.self, from: Data(answer.utf8))
    }
}

@Suite("PushRegistrar")
struct PushRegistrarTests {
    private func register(_ poster: FakePoster, events: [String] = ["request", "device"], device: String? = nil) async throws -> PushRegistrar.Outcome {
        try await PushRegistrar(poster: poster).register(token: "ab12", environment: "sandbox", label: "iPhone", events: events, device: device)
    }

    @Test("a node that knows kinds takes them, in one registration")
    func withKinds() async throws {
        let poster = FakePoster([.success(())])
        #expect(try await register(poster) == .withKinds)
        #expect(poster.calls == [["request", "device"]])
    }

    @Test("a node that refuses `events` with 400 is registered again without them, and is said to send requests only")
    func olderNode() async throws {
        let poster = FakePoster([.failure(NodeError.http(status: 400, message: "unknown field `events`")), .success(())])
        #expect(try await register(poster) == .requestsOnly)
        #expect(poster.calls == [["request", "device"], nil])
    }

    @Test("an empty list is sent as an empty list")
    func none() async throws {
        let poster = FakePoster([.success(())])
        _ = try await register(poster, events: [])
        #expect(poster.calls == [[]])
    }

    @Test("any other failure is the answer and is not sent again", arguments: [
        NodeError.http(status: 401, message: ""), NodeError.http(status: 403, message: "x"), NodeError.http(status: 503, message: "x"), NodeError.sessionExpired,
    ] as [NodeError])
    func otherFailures(error: NodeError) async {
        let poster = FakePoster([.failure(error)])
        await #expect(throws: NodeError.self) { try await register(poster) }
        #expect(poster.calls.count == 1)
    }

    @Test("a second failure after the fallback is the answer")
    func fallbackFails() async {
        let poster = FakePoster([.failure(NodeError.http(status: 400, message: "a")), .failure(NodeError.http(status: 400, message: "b"))])
        await #expect(throws: NodeError.self) { try await register(poster) }
        #expect(poster.calls.count == 2)
    }

    @Test("a registration names its device when it is given one")
    func namesDevice() async throws {
        let poster = FakePoster([.success(())])
        #expect(try await register(poster, device: "d-abc") == .withKinds)
        #expect(poster.devices == ["d-abc"] && poster.calls == [["request", "device"]])
    }

    @Test("a 400 for a registration that names a device is the answer: it is not made again without the device, or without the kinds")
    func deviceRefused() async {
        let poster = FakePoster([.failure(NodeError.http(status: 400, message: "the device is not one of this account's devices")), .success(())])
        await #expect(throws: NodeError.self) { try await register(poster, device: "d-abc") }
        #expect(poster.devices == ["d-abc"], "a registration that went through without the device would not be tied to it")
    }

    @Test("a registration that names no device is as before, and still falls back for a node that does not know kinds")
    func noDevice() async throws {
        let poster = FakePoster([.failure(NodeError.http(status: 400, message: "x")), .success(())])
        #expect(try await register(poster) == .requestsOnly)
        #expect(poster.devices == [nil, nil])
    }
}
