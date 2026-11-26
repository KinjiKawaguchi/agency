import Foundation

/// Lets one read run at a time: a second call while one is running waits for that one instead of starting
/// another. A screen that reads every 30 seconds and can also be pulled down never asks the node twice at once.
@MainActor
final class SingleFlight {
    private var running: Task<Void, Never>?

    func run(_ work: @escaping @MainActor () async -> Void) async {
        if let running {
            await running.value
            return
        }
        let task = Task { await work() }
        running = task
        await task.value
        running = nil
    }
}
