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
    /// The app has left the screen: sends what is due while the system allows, and asks for a later run for what still waits. It starts
    /// at once, in the call that learns the scene has left, because the system's time is asked for before the app is suspended.
    static func finish(_ outbox: DecisionOutbox) {
        guard outbox.hasSignedEntries else { return }
        #if os(iOS)
        let time = SystemTime()
        let flush = Task {
            await outbox.flush()
            scheduleLater(for: outbox)
            time.end()
        }
        time.begin("agency.outbox") {
            // The system's time is up: the send is cut short, a later run is asked for, and the time is given back before this returns, or
            // the system ends the app.
            flush.cancel()
            scheduleLater(for: outbox)
        }
        #else
        Task { await outbox.flush() }
        #endif
    }

    private static func scheduleLater(for outbox: DecisionOutbox) {
        if let retryAt = outbox.retryAt { schedule(at: retryAt) }
    }

    // MARK: A later run

    #if os(iOS)
    /// The time the system gives an app that has just left the screen (`beginBackgroundTask`), given back once: by the work when it is done,
    /// or by the expiry, whichever comes first.
    @MainActor
    private final class SystemTime {
        private var identifier = UIBackgroundTaskIdentifier.invalid

        func begin(_ name: String, onExpiry: @escaping @MainActor () -> Void) {
            identifier = UIApplication.shared.beginBackgroundTask(withName: name) { [self] in
                onExpiry()
                end()
            }
        }

        func end() {
            guard identifier != .invalid else { return }
            UIApplication.shared.endBackgroundTask(identifier)
            identifier = .invalid
        }
    }

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
