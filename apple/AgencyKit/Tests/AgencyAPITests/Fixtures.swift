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

    static func text(_ name: String) throws -> String {
        String(decoding: try data(name), as: UTF8.self)
    }
}
