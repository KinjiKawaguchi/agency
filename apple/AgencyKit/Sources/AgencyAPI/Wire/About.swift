import Foundation

/// What a node says of itself (`GET /api/about`): here only what the app decides by, its capabilities. A reader ignores a field it does
/// not know, and a capability it does not know is just another name.
public struct About: Decodable, Equatable, Sendable {
    /// Features that came after the first release, by name. Strings, so that a name this app has never seen reads.
    public let capabilities: [String]

    public init(capabilities: [String]) {
        self.capabilities = capabilities
    }

    /// Whether the node lists `capability`. A node that does not say is taken not to have it.
    public func has(_ capability: Capability) -> Bool {
        capabilities.contains(capability.rawValue)
    }

    /// The capabilities this app acts on.
    public enum Capability: String, Sendable {
        /// A push registration may name its device (`device`), and the node drops the registrations of a device that is removed.
        case pushDevice
        /// The node serves a team's weekly budget status (`GET /api/orgs/<org>/teams/<team>/budget`).
        case budget
    }
}
