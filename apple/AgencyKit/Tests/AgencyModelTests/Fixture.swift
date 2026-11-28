@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

enum Fixture {
    static func data(_ name: String, from file: String = #filePath) throws -> Data {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/client/\(name).json"))
    }

    static func decode<T: Decodable>(_ type: T.Type, _ name: String) -> T {
        try! JSONDecoder().decode(T.self, from: data(name))
    }
}
