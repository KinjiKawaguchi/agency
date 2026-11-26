import AgencyAPI
import Foundation

/// The two samples of the numbers the Rust tests, the web client and these tests share: made-up figures.
enum OutcomeSamples {
    static func data(_ name: String, from file: String = #filePath) throws -> Data {
        // Tests/AgencyModelTests/Outcomes/OutcomeSamples.swift: six levels up is the repository root.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        return try Data(contentsOf: url.appendingPathComponent("testdata/\(name).sample.json"))
    }

    static func outcomes() throws -> Outcomes { try JSONDecoder().decode(Outcomes.self, from: data("outcomes")) }
    static func readings() throws -> Readings { try JSONDecoder().decode(Readings.self, from: data("readings")) }

    /// A series built from JSON, for the cases the samples do not have.
    static func series(unit: String, currency: String? = nil, goodWhen: String = "higher", before: Double? = nil, points: [(String, Double)]) throws -> KpiSeries {
        func observation(_ day: String, _ value: Double) -> String {
            #"{"kpi":"k","date":"\#(day)","value":\#(value),"source":{"kind":"manual"},"enteredAt":"2026-11-24T12:00:00Z","enteredBy":"owner"}"#
        }
        let currencyField = currency.map { #","currency":"\#($0)""# } ?? ""
        let beforeField = before.map { #","before":\#(observation("2026-10-31", $0))"# } ?? ""
        let list = points.map { observation($0.0, $0.1) }.joined(separator: ",")
        let json = #"{"kpi":{"id":"k","label":"Measure","kind":"business","unit":"\#(unit)"\#(currencyField),"goodWhen":"\#(goodWhen)"},"points":[\#(list)]\#(beforeField)}"#
        return try JSONDecoder().decode(KpiSeries.self, from: Data(json.utf8))
    }
}
