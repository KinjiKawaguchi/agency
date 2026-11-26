import AgencyAPI
import AgencyKit
import Foundation

/// One home node in the switcher: its address, what it is called, and where the entry came from.
public struct HomeNodeEntry: Equatable, Sendable {
    public enum Source: String, Equatable, Sendable, Codable {
        /// The current node's configuration names it (`Me.homeNodes`): the node's to change, not this device's.
        case configured
        /// The owner typed it on this device.
        case added
    }

    public let url: HomeNodeURL
    public let label: String
    public let source: Source

    public init(url: HomeNodeURL, label: String, source: Source) {
        self.url = url
        self.label = label
        self.source = source
    }
}

/// The home nodes to offer besides the current one: the same rule as the web client's card (#330), so the two list the
/// same nodes where they can.
///
/// The nodes the current node names first, in its order; then the ones the owner added on this device. Each origin
/// once, the node's entry winning (its label shown whole), and never the current node. An entry that cannot be opened
/// as a home node (`HomeNodeURL`) is dropped, not trusted. The node tells only its owner; for anyone else `configured`
/// is empty and the list is the device's own.
public enum HomeNodeList {
    /// The most characters of a label the owner types here: the web's own list keeps the same.
    public static let addedLabelLimit = 40

    public static func others(current: HomeNodeURL, configured: [HomeNodeRef], added: [HomeNodeEntry]) -> [HomeNodeEntry] {
        var seen: Set<String> = [origin(of: current)]
        var list: [HomeNodeEntry] = []
        for entry in configured {
            guard let url = try? HomeNodeURL(validating: entry.url), seen.insert(origin(of: url)).inserted else { continue }
            list.append(HomeNodeEntry(url: url, label: label(entry.label, for: url, limit: nil), source: .configured))
        }
        for entry in added where entry.source == .added {
            guard seen.insert(origin(of: entry.url)).inserted else { continue }
            list.append(HomeNodeEntry(url: entry.url, label: label(entry.label, for: entry.url, limit: addedLabelLimit), source: .added))
        }
        return list
    }

    /// What makes two addresses the same node: scheme, host and port, with the scheme's default port filled in, however it was written.
    static func origin(of node: HomeNodeURL) -> String { "\(node.scheme)://\(node.host):\(node.port)" }

    /// The label as the owner or the node wrote it, trimmed; the host when there is none. `limit` cuts a label the owner typed.
    static func label(_ text: String, for url: HomeNodeURL, limit: Int?) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return url.host }
        return limit.map { String(trimmed.prefix($0)) } ?? trimmed
    }
}
