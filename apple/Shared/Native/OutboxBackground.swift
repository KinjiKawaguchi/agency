import AgencyModel
import Foundation
#if os(iOS)
@preconcurrency import BackgroundTasks
import UIKit
#endif

/// What lets a decision kept on the device go on being sent after the app has left the screen: the time the system gives an app that has
/// just left it (iPhone), and a later run it may grant for what is still waiting (`BGAppRefreshTask`). The system decides when, and whether,
/// that run happens; what is not sent then is sent when the app is opened again. A Mac app is not suspended when its window closes, so it
/// only sends.
@MainActor
enum OutboxBackground {
    /// The app has left the screen: sends what is due while the system allows, and asks for a later run for what still waits.
    static func finish(_ outbox: DecisionOutbox) async {
        guard outbox.hasSignedEntries else { return }
        await sendWithTheTimeTheSystemGives(outbox)
        if let retryAt = outbox.retryAt { schedule(at: retryAt) }
    }

    private static func sendWithTheTimeTheSystemGives(_ outbox: DecisionOutbox) async {
        #if os(iOS)
        let flush = Task { await outbox.flush() }
        let held = UIApplication.shared.beginBackgroundTask(withName: "agency.outbox") { flush.cancel() }
        await flush.value
        UIApplication.shared.endBackgroundTask(held)
        #else
        await outbox.flush()
        #endif
    }

    // MARK: A later run

    #if os(iOS)
    /// The identifier the app declares for it (`BGTaskSchedulerPermittedIdentifiers`); none means no later run is asked for.
    private static var identifier: String? {
        (Bundle.main.object(forInfoDictionaryKey: "BGTaskSchedulerPermittedIdentifiers") as? [String])?.first
    }

    /// Called once, before the app finishes launching, as the system requires.
    static func register() {
        guard let identifier else { return }
        BGTaskScheduler.shared.register(forTaskWithIdentifier: identifier, using: nil) { task in
            guard let task = task as? BGAppRefreshTask else { return }
            Task { @MainActor in await run(task) }
        }
    }

    /// Asks the system to wake the app, no sooner than `date`. A refusal (background refresh turned off, too many requests) leaves the
    /// decisions kept: they go when the app is opened.
    static func schedule(at date: Date) {
        guard let identifier else { return }
        let request = BGAppRefreshTaskRequest(identifier: identifier)
        request.earliestBeginDate = date
        try? BGTaskScheduler.shared.submit(request)
    }

    private static func run(_ task: BGAppRefreshTask) async {
        let work = Task { await BackgroundOutbox.flush() }
        task.expirationHandler = { work.cancel() }
        let retryAt = await work.value
        if let retryAt { schedule(at: retryAt) }
        task.setTaskCompleted(success: true)
    }
    #else
    static func schedule(at date: Date) {}
    #endif
}
