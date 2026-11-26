import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

@Suite("Reading line")
struct ReadingLineTests {
    private let kpis = try! OutcomeSamples.outcomes().series.map(\.kpi)
    private let readings = try! OutcomeSamples.readings().readings

    private func line(_ index: Int) -> ReadingLine {
        let reading = readings[index]
        return ReadingLine(reading: reading, team: "App", kpi: kpis.first { $0.id == reading.kpi })
    }

    @Test("a ready reading gives the two numbers and the change, the movement, the line that it is not a cause, and what cannot be told apart")
    func ready() throws {
        let line = line(0)
        #expect(line.epic == "Persistent sessions" && line.team == "App" && line.kpi == "Activation")
        #expect(line.headline == "完了の前 40% → 完了のあと 44%（+4ポイント）")
        let movement = try #require(line.movement)
        #expect(movement.direction == .rose && movement.tone == .better)
        #expect(line.note == "完了のあとで数値が動きました。この仕事が動かしたとは言えません。")
        #expect(line.overlap == "同じ期間に完了した次の仕事とは区別できません: app#12")
    }

    @Test("a ready reading without an overlap says none, and an epic the view no longer lists is named by its reference")
    func readyAlone() {
        let reading = readings[0]
        #expect(ReadingLine(reading: readings[1], team: "App", kpi: kpis[1]).epic == "app#12")
        #expect(readings[0].overlapping == ["app#12"] && reading.state == .ready)
    }

    @Test("every other state says what is missing, gives no number and no movement, and no note")
    func missing() {
        let sentences = (2..<readings.count).map { line($0) }
        for line in sentences {
            #expect(line.movement == nil && line.note == nil && line.overlap == nil, Comment(rawValue: line.headline))
            #expect(!line.headline.contains(where: \.isNumber) || line.headline.contains("日") || line.headline.contains("/"), Comment(rawValue: line.headline))
        }
        #expect(sentences[0].headline == "まだ完了していないため、比べるものがありません。")
        #expect(sentences[1].headline == "完了した日が分からないため、比べられません。")
        #expect(sentences[2].headline.hasPrefix("完了の前の 90 日に数値がありません。") && sentences[2].headline.contains("以前の日付の数値を入れてください。"))
        #expect(sentences[3].headline == "完了のあとの 90 日に数値がなく、比べられません。これから入れても読めません。")
        #expect(sentences[4].headline.hasPrefix("まだ早すぎます。変化が出るまで 14 日かかります。") && sentences[4].headline.hasSuffix("から読めます。"))
        #expect(sentences[5].headline.hasPrefix("完了の前後の数値が足りません: "))
    }

    @Test("the days a reading names are read in Japanese")
    func days() throws {
        let tooEarly = try #require(readings.first { $0.state == .tooEarly })
        let expected = OutcomeFormat.day(try #require(tooEarly.readableFrom))
        #expect(ReadingLine(reading: tooEarly, team: "App", kpi: nil).headline.contains(expected))
        #expect(expected.contains("年"))
    }

    @Test("a state a newer node has, or a ready reading without its numbers, claims nothing")
    func unreadable() throws {
        let unknown = try JSONDecoder().decode(AimReading.self, from: Data(#"{"team":"app","epic":"app#1","kpi":"x","state":"someday"}"#.utf8))
        #expect(ReadingLine(reading: unknown, team: "App", kpi: nil).headline == Ja.Outcomes.unreadable)
        let bare = try JSONDecoder().decode(AimReading.self, from: Data(#"{"team":"app","epic":"app#1","kpi":"x","state":"ready"}"#.utf8))
        let line = ReadingLine(reading: bare, team: "App", kpi: nil)
        #expect(line.headline == Ja.Outcomes.unreadable && line.movement == nil && line.note == nil)
    }

    /// Nothing the page says may say that the work moved a number: a reading is a correlation, never a cause.
    @Test("no word of the page says because, caused, or the work moved it")
    func noCause() {
        let said = (0..<readings.count).flatMap { index -> [String] in
            let line = line(index)
            return [line.headline, line.note, line.overlap].compactMap { $0 }
        } + [
            Ja.Outcomes.note, Ja.Outcomes.readingsHeading, Ja.Outcomes.noAims, Ja.Outcomes.noKpis, Ja.Outcomes.noNumber,
            Ja.Outcomes.unreadable, Ja.Outcomes.rose, Ja.Outcomes.fell, Ja.Outcomes.unchanged, Ja.Outcomes.better, Ja.Outcomes.worse,
            Ja.Outcomes.lastEntered("x"), Ja.Outcomes.lastNumber("1", day: "x"), Ja.Outcomes.title,
        ]
        // "ため" is not on the list: "まだ完了していないため、比べるものがありません" says why nothing can be compared, never why a
        // number moved. What the list holds would claim the work as the cause of a movement.
        let causal = ["おかげ", "せい", "原因", "によって", "効果", "貢献", "ため動", "ため、数値が", "because", "caused", "due to", "thanks to", "result of"]
        for text in said {
            for word in causal {
                #expect(!text.contains(word), Comment(rawValue: "\(word) in: \(text)"))
            }
        }
        #expect(Ja.Outcomes.correlation.contains("動かしたとは言えません"), "the line says it is not a cause")
    }
}
