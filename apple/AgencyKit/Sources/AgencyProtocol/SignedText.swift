import Foundation

/// What a signature is for. The kind is part of the signed text, so a signature made for one kind
/// cannot serve another.
public enum SignedKind: String, Sendable {
    case envelope, org, join, read, device
}

/// The bytes that are signed: `agency/v1/<kind>`, a newline, then the body's canonical JSON.
public func signedText(kind: SignedKind, body: JSONValue) throws -> Data {
    Data("agency/v1/\(kind.rawValue)\n\(try CanonicalJSON.encode(body))".utf8)
}
