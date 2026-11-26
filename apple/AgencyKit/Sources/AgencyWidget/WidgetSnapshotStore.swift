import Foundation

/// Where the snapshot lives: one small file in the app group's container. The app writes it, the widget reads it.
public struct WidgetSnapshotStore: Sendable {
    public static let fileName = "widget.json"
    /// A snapshot is a few hundred bytes; a file much larger than that is not one.
    static let sizeLimit = 64 * 1024

    private let directory: URL

    public init(directory: URL) {
        self.directory = directory
    }

    /// The store in the app group's container; `nil` where there is none (the group is not set up, or not on a phone).
    public static func appGroup(_ identifier: String) -> WidgetSnapshotStore? {
        #if canImport(Darwin)
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier).map(WidgetSnapshotStore.init)
        #else
        nil
        #endif
    }

    private var file: URL { directory.appendingPathComponent(Self.fileName) }

    private static var encoder: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        return encoder
    }

    private static var decoder: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }

    /// Writes the snapshot whole or not at all, so the widget never reads half of one. The file stays readable
    /// after the first unlock, because the widget reads it while the phone is locked.
    public func write(_ snapshot: WidgetSnapshot) throws {
        let data = try Self.encoder.encode(snapshot)
        #if os(iOS)
        try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
        #else
        try data.write(to: file, options: [.atomic])
        #endif
    }

    /// The snapshot, or `nil` when there is none, it is of another version, or it is not one.
    public func read() -> WidgetSnapshot? {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: file.path),
              let size = attributes[.size] as? Int, size <= Self.sizeLimit,
              let data = try? Data(contentsOf: file),
              let snapshot = try? Self.decoder.decode(WidgetSnapshot.self, from: data),
              snapshot.version == WidgetSnapshot.currentVersion
        else { return nil }
        return snapshot
    }

    /// Removes the snapshot, for when the home node is left: nothing of it stays where a widget can show it.
    public func clear() {
        try? FileManager.default.removeItem(at: file)
    }
}
