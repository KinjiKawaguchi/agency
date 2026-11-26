@testable import AgencyAPI
@testable import AgencyModel
import AgencyProtocol
import Foundation
import Testing

/// Ported from `web/src/costs/forecast.test.ts`.
@Suite("Usage window forecast")
struct WindowForecastTests {
    private static let hour: TimeInterval = 3600
    private let now = Date(timeIntervalSince1970: 1_795_521_600)

    private func iso(_ date: Date) -> String { Timestamp.string(from: date) }

    /// A window that is `elapsed` seconds into its length, with the given use so far.
    private func reading(_ name: String, hours: Int, elapsed: TimeInterval, utilization: Int) -> UsageWindow {
        let resets = now.addingTimeInterval(TimeInterval(hours) * Self.hour - elapsed)
        return window(name, hours: hours, utilization: utilization, resetsAt: resets)
    }

    private func window(_ name: String, hours: Int, utilization: Int, resetsAt: Date) -> UsageWindow {
        let json = #"{"name":"\#(name)","lengthHours":\#(hours),"utilization":\#(utilization),"resetsAt":"\#(iso(resetsAt))"}"#
        return try! JSONDecoder().decode(UsageWindow.self, from: Data(json.utf8))
    }

    private func five(elapsed: TimeInterval, _ utilization: Int) -> WindowForecast? {
        WindowForecast(reading: reading("fiveHour", hours: 5, elapsed: elapsed, utilization: utilization), now: now)
    }

    private func week(elapsed: TimeInterval, _ utilization: Int) -> WindowForecast? {
        WindowForecast(reading: reading("sevenDay", hours: 168, elapsed: elapsed, utilization: utilization), now: now)
    }

    @Test("extrapolates the pace so far to the reset")
    func extrapolates() throws {
        let forecast = try #require(five(elapsed: 2.5 * Self.hour, 45))
        #expect(abs(forecast.projectedAtReset - 90) < 0.001)
        #expect(forecast.status == .ok)
        #expect(forecast.hitAt == nil)
    }

    @Test("says when the limit is reached if the pace would overrun the window")
    func overrun() throws {
        let forecast = try #require(five(elapsed: 3 * Self.hour, 74))
        #expect(forecast.status == .overrun)
        #expect(abs(forecast.projectedAtReset - 123.3) < 0.1)
        // 74% took 3 hours, so 100% comes at about 4.05 hours: roughly 1.05 hours from now.
        let hit = try #require(forecast.hitAt)
        #expect(abs(hit.timeIntervalSince(now) - 1.054 * Self.hour) < 10)
    }

    @Test("calls a window that will end well short of full unused")
    func waste() {
        #expect(week(elapsed: 4 * 24 * Self.hour, 30)?.status == .waste)
    }

    @Test("calls a window unused only when it is projected to end under 85%")
    func wasteThreshold() {
        // Half of the weekly window gone, so the projection is twice the use so far.
        #expect(week(elapsed: 3.5 * 24 * Self.hour, 42)?.status == .waste)
        #expect(week(elapsed: 3.5 * 24 * Self.hour, 43)?.status == .ok)
    }

    @Test("treats a full window as run out, with nothing left to predict")
    func full() {
        let forecast = five(elapsed: Self.hour, 100)
        #expect(forecast?.status == .overrun)
        #expect(forecast?.hitAt == nil)
    }

    @Test("does not extrapolate from the first minutes of a window")
    func tooEarly() {
        let forecast = week(elapsed: Self.hour, 5)
        #expect(forecast?.projectedAtReset == 5)
        #expect(forecast?.status == .ok)
    }

    @Test("gives no forecast once the reset has passed, since the reading is out of date")
    func passed() {
        let past = window("fiveHour", hours: 5, utilization: 50, resetsAt: now.addingTimeInterval(-1))
        #expect(WindowForecast(reading: past, now: now) == nil)
    }

    @Test("uses the length the node reports, not one it assumes for the name")
    func ownLength() throws {
        // Two days of a 10-day window gone at 20%: the pace says 100% at the reset.
        let custom = window("sevenDay", hours: 240, utilization: 20, resetsAt: now.addingTimeInterval(8 * 24 * Self.hour))
        let forecast = try #require(WindowForecast(reading: custom, now: now))
        #expect(abs(forecast.projectedAtReset - 100) < 0.001)
    }

    @Test("gives no forecast for a window with no length")
    func noLength() {
        #expect(WindowForecast(reading: window("fiveHour", hours: 0, utilization: 50, resetsAt: now.addingTimeInterval(Self.hour)), now: now) == nil)
    }
}
