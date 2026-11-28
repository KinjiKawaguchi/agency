import Foundation

/// What a node says of itself (`GET /api/about`): what the app decides by, its capabilities and the versions it serves. A reader ignores a
/// field it does not know, and a capability it does not know is just another name.
public struct About: Decodable, Equatable, Sendable {
    /// Features that came after the first release, by name. Strings, so that a name this app has never seen reads.
    public let capabilities: [String]
    /// The Client API versions the node serves; `nil` when it does not say, or says it in a form this app does not read. Such an answer has
    /// no range to compare with, so it is taken as compatible and never as a failure to read the capabilities.
    public let clientApi: ClientApiRange?
    /// For each platform (`ios`, `macos`), the oldest app version the node serves; `nil` while the node sets no floor.
    public let oldestApp: [String: String]?

    public init(capabilities: [String], clientApi: ClientApiRange? = nil, oldestApp: [String: String]? = nil) {
        self.capabilities = capabilities
        self.clientApi = clientApi
        self.oldestApp = oldestApp
    }

    private enum Keys: String, CodingKey { case capabilities, contracts, oldestApp }
    private enum ContractKeys: String, CodingKey { case clientApi }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        capabilities = try container.decode([String].self, forKey: .capabilities)
        // What decides compatibility is read apart from the capabilities: a form this app does not know costs only the comparison.
        let contracts = try? container.nestedContainer(keyedBy: ContractKeys.self, forKey: .contracts)
        clientApi = try? contracts?.decode(ClientApiRange.self, forKey: .clientApi)
        // A floor of one platform that is not a text costs that platform's floor, not the others'.
        oldestApp = (try? container.decodeIfPresent([String: Lenient<String>].self, forKey: .oldestApp))?.compactMapValues(\.value)
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

/// A value that is `nil` when it is not the form asked for, so that one entry of a dictionary does not take the others with it.
private struct Lenient<Value: Decodable & Sendable>: Decodable, Sendable {
    let value: Value?

    init(from decoder: any Decoder) throws {
        value = try? decoder.singleValueContainer().decode(Value.self)
    }
}

/// The Client API versions a node serves: its own (`current`) and the oldest it still answers as (`oldest`).
public struct ClientApiRange: Decodable, Equatable, Sendable {
    public let current: Int
    public let oldest: Int

    public init(current: Int, oldest: Int) {
        self.current = current
        self.oldest = oldest
    }
}

/// The Client API version this app speaks: the one its calls and its reading of the answers are made for. It goes up with a change of the
/// Client API that the app has been made to follow (`docs/compatibility.md`, "Node and apps").
public enum ClientApi {
    public static let spoken = 1
}
