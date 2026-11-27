import Foundation

/// The answers the node really gives, recorded by the Rust tests in `testdata/client/`.
enum Fixtures {
    static func data(_ name: String, from file: String = #filePath) throws -> Data {
        // Tests/AgencyAPITests/Fixtures.swift: five levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/client/\(name).json"))
    }

    /// A sample the Rust tests, the web client and these tests share, in `testdata/<name>.sample.json`.
    static func sample(_ name: String, from file: String = #filePath) throws -> Data {
        // Tests/AgencyAPITests/Fixtures.swift: five levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/\(name).sample.json"))
    }

    /// What earlier nodes answered, kept by `scripts/release/snapshot-data.sh` in `testdata/releases/<tag>/client/`: the
    /// release, the file's name as in `testdata/client/`, and its bytes. Oldest tag first.
    static func earlierAnswers(from file: String = #filePath) throws -> [(tag: String, name: String, data: Data)] {
        // Tests/AgencyAPITests/Fixtures.swift: five levels up is the repository root.
        var root = URL(fileURLWithPath: file)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let releases = root.appendingPathComponent("testdata/releases")
        let manager = FileManager.default
        var answers: [(tag: String, name: String, data: Data)] = []
        for tag in try manager.contentsOfDirectory(atPath: releases.path).sorted() {
            let directory = releases.appendingPathComponent("\(tag)/client")
            guard let files = try? manager.contentsOfDirectory(atPath: directory.path) else { continue }
            for name in files.sorted() where name.hasSuffix(".json") {
                let data = try Data(contentsOf: directory.appendingPathComponent(name))
                answers.append((tag: tag, name: String(name.dropLast(".json".count)), data: data))
            }
        }
        return answers
    }

    static func text(_ name: String) throws -> String {
        String(decoding: try data(name), as: UTF8.self)
    }
}
