#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// Where the owner enters a number of a KPI for a day: the day, the number, an optional note, in which unit to write it, and
/// that a second number for the same day replaces the first (the first stays in the history). What is wrong is said before
/// anything is sent; the node's refusals are said in its words. After a save the form stays open for the next number.
struct EnterNumberSheet: View {
    let card: KpiCard
    /// The node's today, which the day starts at.
    let today: String
    /// Saves a number and says what came of it.
    let onSave: (NumberForm) async -> EntryOutcome
    @Environment(\.dismiss) private var dismiss
    @State private var day: Date
    @State private var value = ""
    @State private var note = ""
    @State private var message: Message?
    @State private var isSaving = false

    private enum Message {
        case saved(String)
        case problem(String)
    }

    init(card: KpiCard, today: String, onSave: @escaping (NumberForm) async -> EntryOutcome) {
        self.card = card
        self.today = today
        self.onSave = onSave
        _day = State(initialValue: Self.date(of: today) ?? Date())
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    DatePicker(Ja.Outcomes.Enter.day, selection: $day, displayedComponents: .date)
                        .environment(\.calendar, Self.calendar)
                        .environment(\.timeZone, Self.calendar.timeZone)
                    TextField(Ja.Outcomes.Enter.value, text: $value)
                        #if os(iOS)
                        .keyboardType(.numbersAndPunctuation)
                        #endif
                        .autocorrectionDisabled()
                    TextField(Ja.Outcomes.Enter.note, text: $note)
                } header: {
                    Text(card.label)
                } footer: {
                    Text("\(Ja.Outcomes.Enter.hint(for: card.unit)) \(Ja.Outcomes.Enter.again)").wrapping()
                }
                if let message {
                    Section { messageText(message) }
                }
            }
            .navigationTitle(Ja.Outcomes.Enter.open)
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(Ja.Outcomes.Enter.close) { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(isSaving ? Ja.Outcomes.Enter.saving : Ja.Outcomes.Enter.save, action: save).disabled(isSaving)
                }
            }
        }
        .coveredWhenInactive()
        .presentationDetents([.medium, .large])
    }

    @ViewBuilder
    private func messageText(_ message: Message) -> some View {
        switch message {
        case .saved(let text): Text(text).font(.subheadline).foregroundStyle(.green).wrapping()
        case .problem(let text): Text(text).font(.subheadline).foregroundStyle(Theme.danger).wrapping()
        }
    }

    private func save() {
        isSaving = true
        message = nil
        let form = NumberForm(date: Self.text(of: day), value: value, note: note)
        Task {
            let outcome = await onSave(form)
            isSaving = false
            switch outcome {
            case .saved(let text):
                message = .saved(text)
                value = ""
                note = ""
            case .problem(let problem): message = .problem(problem.text)
            case .failed(let text): message = .problem(text)
            }
        }
    }

    // The day is a calendar day with no time of its own: it is carried as a date in UTC, so that the day picked is the day sent.
    private static var calendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC") ?? .gmt
        return calendar
    }

    static func date(of day: String) -> Date? {
        let parts = day.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return nil }
        return calendar.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
    }

    static func text(of date: Date) -> String {
        // Written by a formatter of the same UTC calendar, so no part of the day can be missing and need a default.
        date.formatted(Date.ISO8601FormatStyle(timeZone: calendar.timeZone).year().month().day())
    }
}
#endif
