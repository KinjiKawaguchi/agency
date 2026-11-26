import Foundation

/// A change others asked the org's root key to sign, which its node holds until the owner signs it there (`agency root-sign`),
/// as `/api/me` lists it for the owner. The kind of change and the fields it is worded from are optional and open: an app
/// words a kind it knows from them, and says the node's own `what` for any other kind or when a field is missing.
public struct RootChangeRef: Decodable, Equatable, Sendable {
    public let id: String
    public let org: String
    /// What it changes, in the node's English words ("add member carol (Carol)"). The node always gives it; it is read as optional
    /// so that an entry the app can word from its kind and fields is not lost if a node leaves it out.
    public let what: String?
    /// When the node took the request (an RFC 3339 time, as the node wrote it).
    public let receivedAt: String
    /// `addMember`, `addDevice` or `removeDevice` for the kinds an app words itself; any other value is a kind it does not know.
    public let kind: String?
    /// The member it is for: the joiner for `addMember`; the owner of the device for the other two.
    public let member: String?
    /// The member's label for `addMember`; the device's label for `addDevice`.
    public let label: String?
    /// The device's id for `addDevice` and `removeDevice`: so that the device an owner approved is known to be held.
    public let device: String?

    public init(
        id: String, org: String, what: String? = nil, receivedAt: String,
        kind: String? = nil, member: String? = nil, label: String? = nil, device: String? = nil
    ) {
        self.id = id
        self.org = org
        self.what = what
        self.receivedAt = receivedAt
        self.kind = kind
        self.member = member
        self.label = label
        self.device = device
    }

    private enum Keys: String, CodingKey { case id, org, what, receivedAt, kind, member, label, device }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        func text(_ key: Keys) -> String? {
            (try? container.decodeIfPresent(String.self, forKey: key)).flatMap { $0 }.flatMap { $0.isEmpty ? nil : $0 }
        }
        guard let id = text(.id), let org = text(.org), let receivedAt = text(.receivedAt) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "A held root change names no id, org or time."))
        }
        self.init(
            id: id, org: org, what: text(.what), receivedAt: receivedAt,
            kind: text(.kind), member: text(.member), label: text(.label), device: text(.device)
        )
    }
}

extension Me {
    /// The held changes of `rootChanges`, as the list's own entries: an entry that is not one is left out, and a list that is not
    /// a list is none, so that the owner's account still reads when the node words this part differently.
    static func rootChanges(in container: KeyedDecodingContainer<Keys>) -> [RootChangeRef] {
        struct Entry: Decodable {
            let value: RootChangeRef?
            init(from decoder: any Decoder) { value = try? RootChangeRef(from: decoder) }
        }
        let entries = (try? container.decodeIfPresent([Entry].self, forKey: .rootChanges)) ?? nil
        return entries?.compactMap(\.value) ?? []
    }
}
