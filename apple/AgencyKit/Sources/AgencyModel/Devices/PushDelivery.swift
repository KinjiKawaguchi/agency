import AgencyAPI
import Foundation

/// What the node knows of how APNs took the notices it sent this phone's token (agency#386 for the browsers, the same facts for the
/// phone): nothing until APNs has taken or refused one, `taken` with the time of the last it took, or `failing` when its last refusal is
/// later than its last taking, or it has taken none.
public enum PushDelivery: Equatable, Sendable {
    case unknown
    case taken(ReportedTime)
    /// The last refusal, with when and why, and when the last notice was taken when there was one: "never" is a missing value, and not a
    /// date of 0.
    case failing(at: ReportedTime, reason: String, lastTakenAt: ReportedTime?)

    public init(token: RegisteredPushToken) {
        switch (token.lastSucceededAt, token.lastFailure) {
        case (nil, nil): self = .unknown
        case (let taken?, nil): self = .taken(taken)
        case (nil, let failure?): self = .failing(at: failure.at, reason: failure.reason, lastTakenAt: nil)
        case (let taken?, let failure?):
            // A taking at the same time as the refusal, or after it, means the notices go through again. A time that cannot be read is not a
            // later time: the refusal is then what stands, since it is the one that tells something is wrong.
            if let takenAt = taken.date, let failedAt = failure.at.date, takenAt >= failedAt {
                self = .taken(taken)
            } else {
                self = .failing(at: failure.at, reason: failure.reason, lastTakenAt: taken)
            }
        }
    }

    /// Whether what the screen says is a trouble: only a notice that APNs did not take is.
    public var isFailing: Bool {
        if case .failing = self { true } else { false }
    }

    /// What the Devices screen says: in the web's words when the notices are failing, and nothing otherwise: when APNs takes them (a
    /// refusal older than a taking is not one) and when the node has nothing to say, which is not "refused" (it may never have sent a
    /// notice, or be an older node that records none).
    public func lines(in timeZone: TimeZone = .current) -> [String]? {
        guard case .failing(let at, let reason, let lastTakenAt) = self else { return nil }
        func when(_ time: ReportedTime) -> String { time.date.map { JaFormat.dateTime($0, in: timeZone) } ?? time.text }
        let last = lastTakenAt.map { Ja.PushDelivery.lastTaken(when($0)) } ?? Ja.PushDelivery.neverTaken
        return [Ja.PushDelivery.isFailing(when(at), reason), last]
    }

    /// The token of this phone among those the node lists, which it shortens to their last 8 characters.
    public static func ofOwn(token own: String, in listed: [RegisteredPushToken]) -> PushDelivery? {
        let tail = String(own.lowercased().suffix(8))
        return listed.first { $0.token.lowercased() == tail }.map(PushDelivery.init(token:))
    }
}
