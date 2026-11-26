import Foundation

/// `GET /api/orgs/<org>`: the org's signed definition, as far as a screen draws it: its departments, its members
/// and the teams placed in them. Keys, devices and versions are not read here.
public struct OrgDefinition: Decodable, Equatable, Sendable {
    public let org: String
    public let label: String
    public let departments: [OrgDepartment]
    public let people: [OrgMember]
    public let teams: [TeamPlace]

    public init(org: String, label: String, departments: [OrgDepartment], people: [OrgMember], teams: [TeamPlace]) {
        self.org = org
        self.label = label
        self.departments = departments
        self.people = people
        self.teams = teams
    }
}

/// A unit of the org that groups members and teams, with an optional parent and an optional head (a member).
public struct OrgDepartment: Decodable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let parent: String?
    public let head: String?

    public init(id: String, label: String, parent: String? = nil, head: String? = nil) {
        self.id = id
        self.label = label
        self.parent = parent
        self.head = head
    }
}

/// An account's seat in the org (`Person` in the protocol).
public struct OrgMember: Decodable, Equatable, Sendable {
    /// `member` reads and answers; `answerer` only answers.
    public enum Role: String, Decodable, Sendable { case member, answerer }

    public let id: String
    public let label: String
    public let account: String
    public let department: String?
    public let role: Role

    public init(id: String, label: String, account: String, department: String? = nil, role: Role = .member) {
        self.id = id
        self.label = label
        self.account = account
        self.department = department
        self.role = role
    }
}

/// A team and the node it runs on.
public struct TeamPlace: Decodable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let department: String?
    public let node: String

    public init(id: String, label: String, department: String? = nil, node: String = "") {
        self.id = id
        self.label = label
        self.department = department
        self.node = node
    }
}
