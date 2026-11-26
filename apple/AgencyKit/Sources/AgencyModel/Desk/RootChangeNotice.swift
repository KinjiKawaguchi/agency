import AgencyAPI
import AgencyProtocol
import Foundation

/// The changes others asked the root key to sign, which wait for the owner, as the desk says them: one item with how many they
/// are, what each changes, and the command that signs them together. The node tells the owner of them in `/api/me`; the
/// item shows them for as long as the node does, and there is none when none waits.
public struct RootChangeNotice: Equatable, Sendable {
    public struct Item: Equatable, Identifiable, Sendable {
        public let id: String
        /// What it changes, in the app's words from its kind and fields, or the node's own words.
        public let what: String
        /// The org's label, or its id when the account does not list the org.
        public let org: String
        /// `nil` when the node's time cannot be read.
        public let receivedAt: Date?
    }

    /// The command the owner runs on the root key's node. It is the app's own word, never the node's.
    public static let command = "agency root-sign"

    /// What a device's request says once this device approved it and the root's node holds the change.
    public static var approvedHere: String { Ja.RootChanges.approvedHere(command: command) }

    public let items: [Item]

    /// `nil` when nothing waits, so that nothing is drawn.
    public init?(me: Me) {
        let items = me.rootChanges.compactMap { change in
            Self.words(of: change).map { Item(id: change.id, what: $0, org: Self.label(of: change.org, in: me.orgs), receivedAt: Timestamp.date(from: change.receivedAt)) }
        }
        guard !items.isEmpty else { return nil }
        self.items = items
    }

    public var title: String { Ja.RootChanges.title(count: items.count) }

    /// One line for each change.
    public func lines(now: Date) -> [String] {
        items.map { item in
            guard let receivedAt = item.receivedAt else { return Ja.RootChanges.itemWithoutTime(item.what, org: item.org) }
            return Ja.RootChanges.item(item.what, org: item.org, when: Ago.text(from: receivedAt, now: now))
        }
    }

    /// The sentence that gives the command, in the parts around it, so that a screen can set the command apart.
    public var signTogether: (before: String, command: String, after: String) {
        (Ja.RootChanges.signTogetherBefore, Self.command, Ja.RootChanges.signTogetherAfter)
    }

    /// The app's words for a kind it knows, from the fields that kind needs; the node's `what` for any other kind or when a field
    /// is missing; none when the node gave neither.
    static func words(of change: RootChangeRef) -> String? {
        switch (change.kind, change.member, change.label, change.device) {
        case ("addMember", let member?, let label?, _): Ja.RootChanges.addMember(label: label, member: member)
        case ("addDevice", let member?, let label?, _): Ja.RootChanges.addDevice(label: label, member: member)
        case ("removeDevice", let member?, _, let device?): Ja.RootChanges.removeDevice(device: device, member: member)
        default: change.what
        }
    }

    private static func label(of org: String, in orgs: [OrgRef]) -> String {
        orgs.first { $0.id == org }?.label ?? org
    }
}
