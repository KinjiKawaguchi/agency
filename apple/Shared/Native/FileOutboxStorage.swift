import AgencyModel
import Foundation
import os

/// The decisions kept on this device until the node has taken them (`DecisionOutbox`), one file for each home node, in Application Support.
///
/// A write replaces the file whole and atomically, so a kill in the middle leaves the old file or the new one. The file can be read after
/// the first unlock (so a background send can read it with the phone locked) and is left out of backups: it holds decisions and their
/// text, and nothing but this device sends them.
struct FileOutboxStorage: OutboxStorage {
    private static let logger = Logger(subsystem: "agency.app", category: "outbox")
    private let url: URL

    init(host: String) {
        url = Self.url(host: host)
    }

    private static func url(host: String) -> URL {
        let directory = (try? FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true))
            ?? FileManager.default.temporaryDirectory
        let safe = host.map { $0.isLetter || $0.isNumber || $0 == "." || $0 == "-" ? String($0) : "_" }.joined()
        return directory.appendingPathComponent("agency", isDirectory: true).appendingPathComponent("outbox-\(safe).json")
    }

    /// Removes what is kept for the home node at `host` (it is forgotten).
    static func forget(host: String) {
        try? FileManager.default.removeItem(at: url(host: host))
    }

    /// Only "there is no file" is an empty outbox. A file that exists and cannot be read now (protected until the first unlock, an I/O
    /// error) throws: the outbox then keeps nothing new, because writing would replace what is in the file.
    func load() throws -> [PendingDecision] {
        let data: Data
        do {
            data = try Data(contentsOf: url)
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile {
            return []
        }
        do {
            return try JSONDecoder().decode([PendingDecision].self, from: data)
        } catch {
            // A file that cannot be read is kept aside, not overwritten: what is in it is the owner's decisions.
            Self.logger.error("The outbox file could not be read; it is kept as outbox.corrupt")
            try? FileManager.default.removeItem(at: url.appendingPathExtension("corrupt"))
            try? FileManager.default.moveItem(at: url, to: url.appendingPathExtension("corrupt"))
            return []
        }
    }

    func save(_ entries: [PendingDecision]) throws {
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(entries)
        #if os(iOS)
        try data.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
        #else
        try data.write(to: url, options: [.atomic])
        #endif
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        var mutable = url
        try? mutable.setResourceValues(values)
    }
}
