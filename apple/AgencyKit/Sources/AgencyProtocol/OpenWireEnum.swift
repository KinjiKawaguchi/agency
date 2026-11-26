import Foundation

/// A word of a node's (or a team runtime's) that the app reads even when it does not know it: an enum of such words has a case
/// `unknown`, and a word that is not one of the others is that case, so that a newer node's new word neither makes the whole
/// answer unreadable (`docs/compatibility.md`: readers ignore what they do not know) nor is taken for one of the words the
/// app does know, a success among them. Where the app has to do something with it, it does the safe thing for `unknown`.
public protocol OpenWireEnum: RawRepresentable, Decodable, Sendable where RawValue == String {
    /// Any word the app does not know.
    static var unknown: Self { get }
}

extension OpenWireEnum {
    public init(from decoder: any Decoder) throws {
        let word = try decoder.singleValueContainer().decode(String.self)
        self = Self(rawValue: word) ?? Self.unknown
    }
}
