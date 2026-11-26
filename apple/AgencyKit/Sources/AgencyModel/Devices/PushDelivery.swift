import AgencyAPI
import Foundation

/// What the node knows of how APNs took the notices it sent this phone's token (agency#386 for the browsers, the same facts for the
/// phone): nothing until APNs has taken or refused one, `taken` with the time of the last it took, or `failing` when its last refusal is
/// later than its last taking, or it has taken none.
public enum PushDelivery: Equatable, Sendable {
    case unknown
    case taken(Date)
    /// The last refusal, with when and why, and when the last notice was taken when there was one: "never" is a missing value, and not a
    /// date of 0.
    case failing(at: Date, reason: String, lastTakenAt: Date?)

    public init(token: RegisteredPushToken) {
        switch (token.lastSucceededAt, token.lastFailure) {
        case (nil, nil): self = .unknown
        case (let taken?, nil): self = .taken(taken.date)
        case (nil, let failure?): self = .failing(at: failure.at.date, reason: failure.reason, lastTakenAt: nil)
        case (let taken?, let failure?):
            // A taking at the same time as the refusal, or after it, means the notices go through again.
            self = taken.date >= failure.at.date ? .taken(taken.date) : .failing(at: failure.at.date, reason: failure.reason, lastTakenAt: taken.date)
        }
    }

    /// What the Devices screen says, in the web's words, when the notices are failing; nothing when they are taken or nothing is known.
    public func lines(in timeZone: TimeZone = .current) -> [String]? {
        guard case .failing(let at, let reason, let lastTakenAt) = self else { return nil }
        let last = lastTakenAt.map { Ja.PushDelivery.lastTaken(JaFormat.dateTime($0, in: timeZone)) } ?? Ja.PushDelivery.neverTaken
        return [Ja.PushDelivery.isFailing(JaFormat.dateTime(at, in: timeZone), reason), last]
    }

    /// The token of this phone among those the node lists, which it shortens to their last 8 characters.
    public static func ofOwn(token own: String, in listed: [RegisteredPushToken]) -> PushDelivery? {
        let tail = String(own.lowercased().suffix(8))
        return listed.first { $0.token.lowercased() == tail }.map(PushDelivery.init(token:))
    }
}
