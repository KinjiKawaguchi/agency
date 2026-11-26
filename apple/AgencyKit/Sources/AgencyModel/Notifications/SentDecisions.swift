import Foundation

/// The ids of the decisions this phone sent in this session, so that the success of a decision it sent
/// is not announced to it as news (a failure is). It forgets them when the app closes: after that, the
/// first reading is a baseline and nothing old is announced anyway.
public final class SentDecisions: @unchecked Sendable {
    private let lock = NSLock()
    private var recorded: Set<String> = []

    public init() {}

    public func record(_ id: String) {
        lock.withLock { _ = recorded.insert(id) }
    }

    public var ids: Set<String> { lock.withLock { recorded } }
}
