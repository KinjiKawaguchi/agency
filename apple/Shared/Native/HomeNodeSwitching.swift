import AgencyAPI
import AgencyKit
import AgencyModel

/// What a shell asks of whoever holds the home nodes: the iPhone's root view (through the store) or the Mac's app model
/// (which also has the session to replace).
@MainActor
protocol HomeNodeSwitching: AnyObject {
    var others: [HomeNodeEntry] { get }
    /// Go to another node: its sign-in, key and choices are kept as they were.
    func switchTo(_ node: HomeNodeURL)
    /// An address the owner entered, to be on the list; going to it is a switch.
    func add(_ node: HomeNodeURL, label: String)
    /// Take a node off this device, with its sign-in, its key and its choices.
    func forget(_ node: HomeNodeURL)
    /// Leave the current node: the setup comes back.
    func leave()
    /// What the current node says about the owner's other nodes.
    func note(configured: [HomeNodeRef])
    /// The id the current node says it puts in its pushes.
    func note(nodeID: String?, of node: HomeNodeURL)
    /// The id a push from `node` carries.
    func nodeID(of node: HomeNodeURL) -> String
}

extension HomeNodeStore: HomeNodeSwitching {
    func leave() { clear() }
}
