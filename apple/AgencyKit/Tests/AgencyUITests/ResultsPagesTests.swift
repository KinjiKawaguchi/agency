#if canImport(SwiftUI) && os(macOS)
import Testing
@testable import AgencyUI

@Suite("Results pages")
struct ResultsPagesTests {
    @Test("the outcomes page is on offer only when the owner may read it, and nothing else changes")
    func offered() {
        #expect(ResultsScreen.pages(isOutcomesAvailable: false) == [.costs, .evaluation])
        #expect(ResultsScreen.pages(isOutcomesAvailable: true) == [.costs, .evaluation, .outcomes])
    }

    @Test("when the outcomes page goes away while it is chosen, the costs are shown and selected, not a selection no page has")
    func goesAway() {
        #expect(ResultsScreen.shown(.outcomes, isOutcomesAvailable: true) == .outcomes)
        #expect(ResultsScreen.shown(.outcomes, isOutcomesAvailable: false) == .costs)
        #expect(ResultsScreen.shown(.evaluation, isOutcomesAvailable: false) == .evaluation)
    }
}
#endif
