import AgencyKit
import AgencyModel
import Foundation
import Observation

/// What the Mac app keeps whether or not its window is open: the home node, and the shell (so the session) for it.
/// The window and the menu-bar item both show this one. Closing the window destroys a view, never the session, so
/// the desk is still read every 30 seconds and the menu-bar item keeps its number.
@MainActor
@Observable
final class MacAppModel {
    let store: HomeNodeStore
    let push: PushCoordinator
    private(set) var shell: ShellModel?
    @ObservationIgnored private var reading: Task<Void, Never>?

    init(push: PushCoordinator, store: HomeNodeStore = HomeNodeStore()) {
        self.push = push
        self.store = store
    }

    /// Opens the shell for the saved home node, when there is one: at launch.
    func begin() {
        guard shell == nil, let homeNode = store.homeNode else { return }
        open(homeNode)
    }

    /// The owner entered the home node's address.
    func save(_ homeNode: HomeNodeURL) {
        store.save(homeNode)
        open(homeNode)
    }

    /// How many requests wait for the owner: `nil` until the node has answered (and with no home node), 0 when it has and nothing waits.
    var waitingCount: Int? { shell?.session?.desk.knownWaitingCount }

    private func open(_ homeNode: HomeNodeURL) {
        reading?.cancel()
        let shell = ShellModel(
            homeNode: homeNode, push: push, deviceLabel: Ja.Device.macLabel,
            onChangeHomeNode: { [weak self] in self?.leave() }
        )
        self.shell = shell
        shell.start()
        keepReading(shell)
    }

    /// The home node is left: the shell and its session go, and the setup comes back.
    private func leave() {
        reading?.cancel()
        shell = nil
        store.clear()
    }

    /// Reads the desk every 30 seconds for as long as the app runs, with or without a window, so that the menu-bar
    /// item and the Dock badge follow it. (The news is polled by the window: a banner nobody can see is not news.)
    private func keepReading(_ shell: ShellModel) {
        reading = Task { [weak shell, push] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                guard !Task.isCancelled, let desk = shell?.session?.desk else { continue }
                await desk.refresh()
                if let count = desk.knownWaitingCount { push.setBadge(count) }
            }
        }
    }
}
