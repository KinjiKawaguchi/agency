import AgencyProtocol
import Foundation

/// A protocol timestamp (RFC 3339 in UTC). The text is kept as the node wrote it.
public struct Instant: Decodable, Equatable, Hashable, Comparable, Sendable {
    public let text: String
    public let date: Date

    public init(from decoder: any Decoder) throws {
        let text = try decoder.singleValueContainer().decode(String.self)
        guard let date = Timestamp.date(from: text) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "not an RFC 3339 time: \(text)"))
        }
        self.text = text
        self.date = date
    }

    public static func < (left: Instant, right: Instant) -> Bool { left.date < right.date }
}
