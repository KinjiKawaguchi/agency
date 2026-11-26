import AgencyAPI
import AgencyProtocol
import Foundation

/// A recovery of an org that was started and waits, as the desk says it (agency#360). The node tells the owner of it in
/// `/api/me`; the desk shows one for each such org, at the top, with no way to dismiss it, for as long as the node says it waits.
public struct RecoveryNotice: Equatable, Identifiable, Sendable {
    public var id: String { orgID }
    /// The org's id, which the cancel command takes.
    public let orgID: String
    /// The org's own label, which the owner knows it by.
    public let label: String
    /// When it takes effect on the node, when the node says and the time can be read.
    public let takesEffectAt: Date?
    /// The fingerprint of the root key it moves the org to; none when the root key stays.
    public let newRootKey: String?
    public let replacesRecoveryKey: Bool
    /// Whether the viewer is the owner of the org's root key (`OrgRef.isRoot`): only then can they cancel it, so only then does
    /// the notice give the command. Every member is told that a root change started: it concerns everyone whose trust follows it.
    public let isRoot: Bool

    public init(org: OrgRef, recovery: StartedRecovery) {
        isRoot = org.isRoot
        orgID = org.id
        label = org.label
        takesEffectAt = recovery.takesEffectAt.flatMap { Timestamp.date(from: $0) }
        newRootKey = recovery.newRootKey
        replacesRecoveryKey = recovery.replacesRecoveryKey
    }

    /// The recoveries of the account's orgs, in the account's order.
    public static func all(in orgs: [OrgRef]) -> [RecoveryNotice] {
        orgs.compactMap { org in org.recovery.map { RecoveryNotice(org: org, recovery: $0) } }
    }

    /// What the notice says before the command: what started, when it takes effect (when known), and the key it moves to.
    public func lines(in timeZone: TimeZone = .current) -> [String] {
        var lines = [isRoot ? Ja.Recovery.title(org: label) : Ja.Recovery.titleForMember(org: label)]
        if let takesEffectAt { lines.append(Ja.Recovery.takesEffect(at: JaFormat.dateTime(takesEffectAt, in: timeZone))) }
        if let newRootKey { lines.append(Ja.Recovery.newRootKey(newRootKey)) }
        if replacesRecoveryKey { lines.append(Ja.Recovery.replacesRecoveryKey) }
        if !isRoot { lines.append(Ja.Recovery.ownerCanCancel) }
        return lines
    }

    /// What to run on the root key's node to cancel it; none for a viewer who cannot.
    public var cancelCommand: String? { isRoot ? Ja.Recovery.cancelCommand(orgID: orgID) : nil }
}
