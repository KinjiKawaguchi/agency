import AgencyAPI
@testable import AgencyModel
import Testing

@Suite("Number check")
struct NumberCheckTests {
    private let today = "2026-11-24"

    private func form(date: String = "2026-11-20", value: String = "1350000", note: String = "") -> NumberForm {
        NumberForm(date: date, value: value, note: note)
    }

    private func check(_ form: NumberForm) -> Result<ObservationInput, NumberProblem> {
        NumberCheck.check(form, kpi: "revenue", today: today)
    }

    @Test("a number for a day is taken, with or without a note, trimmed")
    func takes() {
        #expect(check(form()) == .success(ObservationInput(kpi: "revenue", date: "2026-11-20", value: 1_350_000)))
        #expect(check(form(value: " -3.5 ", note: "  before the invoices  "))
            == .success(ObservationInput(kpi: "revenue", date: "2026-11-20", value: -3.5, note: "before the invoices")))
        #expect(check(form(value: "0")) == .success(ObservationInput(kpi: "revenue", date: "2026-11-20", value: 0)), "a 0 typed is a 0")
        for (written, value) in [("5.", 5.0), (".5", 0.5), ("+7", 7.0), ("-0.25", -0.25), ("007", 7.0)] {
            #expect(check(form(value: written)) == .success(ObservationInput(kpi: "revenue", date: "2026-11-20", value: value)), Comment(rawValue: written))
        }
    }

    @Test("a date that is not a real day, not a day at all, empty, too far ahead or too old is refused")
    func refusesDates() {
        for date in ["2026-02-30", "yesterday", "", "2026-11-26", "2016-11-23", "2026-1-5", "２０２６-11-20", "2026-11-20 ", "2026/11/20", "2026-13-01", "2026-00-10"] {
            #expect(check(form(date: date)) == .failure(.date), Comment(rawValue: date))
        }
    }

    @Test("a date one day ahead and one exactly ten years back are taken")
    func dateEdges() {
        #expect(check(form(date: "2026-11-25")) != .failure(.date))
        #expect(check(form(date: "2016-11-24")) != .failure(.date))
        #expect(check(form(date: "2026-11-24")) != .failure(.date))
    }

    @Test("the ten years back of a February 29 is March 1, as the web counts it")
    func leapDay() {
        #expect(NumberCheck.check(form(date: "2016-03-01"), kpi: "k", today: "2026-02-28") != .failure(.date))
        #expect(NumberCheck.check(form(date: "2016-02-28"), kpi: "k", today: "2026-02-28") != .failure(.date))
        #expect(NumberCheck.check(form(date: "2016-02-27"), kpi: "k", today: "2026-02-28") == .failure(.date))
    }

    @Test("a value that is not digits, a sign and a point is refused: not hex, not an exponent, not infinity, not grouped")
    func refusesValues() {
        for value in ["", "  ", "abc", "NaN", "nan", "Infinity", "inf", "1e999", "1,5", "1e15000", "1000000000000001", "0x1F", "1e3", "1_000", "--1", "1.2.3", ".", "+", "-", "１２", "1 000"] {
            #expect(check(form(value: value)) == .failure(.value), Comment(rawValue: value))
        }
    }

    @Test("the largest value the node takes is taken, and one past it is not")
    func largest() {
        #expect(check(form(value: "1000000000000000")) != .failure(.value))
        #expect(check(form(value: "-1000000000000000")) != .failure(.value))
        #expect(check(form(value: "1000000000000000.5")) == .failure(.value))
    }

    @Test("a note of more than 200 characters or with a control character is refused, and the characters are counted, not the bytes")
    func notes() {
        #expect(check(form(note: String(repeating: "a", count: 201))) == .failure(.note))
        #expect(check(form(note: "a\u{07}b")) == .failure(.note))
        #expect(check(form(note: "a\nb")) == .failure(.note))
        #expect(check(form(note: "a\u{7F}b")) == .failure(.note))
        #expect(check(form(note: "a\u{1F}b")) == .failure(.note), "the last of the control characters below the space")
        #expect(check(form(note: "a b")) != .failure(.note), "a space inside a note is fine")
        #expect(check(form(note: String(repeating: "😀", count: 200))) != .failure(.note))
        #expect(check(form(note: String(repeating: "あ", count: 200))) != .failure(.note))
        #expect(check(form(note: String(repeating: "a", count: 200))) != .failure(.note))
        #expect(check(form(note: String(repeating: "😀", count: 201))) == .failure(.note))
        #expect(check(form(note: "\n")) == .success(ObservationInput(kpi: "revenue", date: "2026-11-20", value: 1_350_000)), "a note of only spaces is none")
    }

    @Test("each problem has its words, the web's")
    func words() {
        #expect(NumberProblem.date.text.hasPrefix("日付は実在する日で"))
        #expect(NumberProblem.value.text.contains("1,000,000,000,000,000"))
        #expect(NumberProblem.note.text.contains("200 文字"))
    }
}
