import Foundation

/// What the app observed of one call to its home node: the route as the node names it, how the call ended, and how long it took.
public struct CallSample: Equatable, Sendable {
    /// The route's template (`GET /api/orgs/{org}/teams/{team}/view`): never a path with an id, a query, a header or a body.
    public let route: String
    /// `2xx`, `3xx`, `4xx`, `5xx`, or `network` for a call that got no answer.
    public let status: String
    /// Whole milliseconds from the start of the request until its body was read, or until the failure came.
    public let ms: Int

    public init(route: String, status: String, ms: Int) {
        self.route = route
        self.status = status
        self.ms = ms
    }

    public static let network = "network"

    /// The class of an HTTP status, or `nil` for one outside 200 to 599, which the node would drop.
    public static func statusClass(_ status: Int) -> String? {
        (200...599).contains(status) ? "\(status / 100)xx" : nil
    }
}

/// Times every call that goes through it, and tells `observe` what it saw. It adds nothing to a call and waits for nothing: `observe` returns
/// at once, and a call the caller itself cancelled is not observed.
public struct TimedTransport: HTTPTransport {
    private let base: any HTTPTransport
    private let observe: @Sendable (CallSample) -> Void

    public init(_ base: any HTTPTransport, observe: @escaping @Sendable (CallSample) -> Void) {
        self.base = base
        self.observe = observe
    }

    public func send(_ request: HTTPRequest) async throws -> HTTPResponse {
        let clock = ContinuousClock()
        let started = clock.now
        do {
            let response = try await base.send(request)
            note(request, status: CallSample.statusClass(response.status), since: started, clock: clock)
            return response
        } catch {
            if !Self.isCancellation(error) { note(request, status: CallSample.network, since: started, clock: clock) }
            throw error
        }
    }

    private func note(_ request: HTTPRequest, status: String?, since started: ContinuousClock.Instant, clock: ContinuousClock) {
        guard let status, let route = RouteTemplates.template(method: request.method, path: request.path) else { return }
        let elapsed = started.duration(to: clock.now)
        let milliseconds = elapsed.components.seconds * 1_000 + elapsed.components.attoseconds / 1_000_000_000_000_000
        observe(CallSample(route: route, status: status, ms: Int(max(0, milliseconds))))
    }

    private static func isCancellation(_ error: Error) -> Bool {
        error is CancellationError || (error as? URLError)?.code == .cancelled
    }
}
