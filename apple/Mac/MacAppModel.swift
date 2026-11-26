import AgencyAPI
import AgencyKit
import AgencyModel
import Foundation
import Observation

/// What the Mac app keeps whether or not its window is open: the home node, and the shell (so the session) for it.
/// The window and the menu-bar item both show this one. Closing the window destroys a view, never the session, so
/// the desk is still read every 30 seconds and the menu-bar item keeps its number.
@MainActor
@Observable
final class MacAppModel: HomeNodeSwitching {
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

    /// What the menu-bar item says: no home node, not read yet, a read that failed, or the last number the node gave (0 included).
    var menuState: MenuBarState {
        let desk = shell?.session?.desk
        return MenuBarState(hasHomeNode: store.homeNode != nil, known: desk?.knownWaitingCount, isFailed: desk?.isReadFailing == true)
    }

    // MARK: The owner's other home nodes (the shell asks)

    var others: [HomeNodeEntry] { store.others }

    func switchTo(_ node: HomeNodeURL) {
        store.switchTo(node)
        open(node)
    }

    func add(_ node: HomeNodeURL, label: String) { store.add(node, label: label) }
    func note(configured: [HomeNodeRef]) { store.note(configured: configured) }

    func forget(_ node: HomeNodeURL) { store.forget(node) }

    private func open(_ homeNode: HomeNodeURL) {
        reading?.cancel()
        let shell = ShellModel(
            homeNode: homeNode, push: push, deviceLabel: Ja.Device.macLabel,
            nodes: self
        )
        self.shell = shell
        shell.start()
        keepReading(shell)
    }

    /// The home node is left: the shell and its session go, and the setup comes back.
    func leave() {
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
