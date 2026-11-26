#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The results tab: the costs and the evaluation, behind a switch at the top, as the web client groups them.
public struct ResultsScreen: View {
    private let costs: CostsStore
    private let evaluation: EvaluationStore
    private let outcomes: OutcomesStore
    private let onSignInAgain: () -> Void
    @State private var page: Page = .costs

    enum Page: CaseIterable, Hashable {
        case costs, evaluation, outcomes

        var title: String {
            switch self {
            case .costs: Ja.Tabs.costs
            case .evaluation: Ja.Evaluation.title
            case .outcomes: Ja.Outcomes.title
            }
        }
    }

    /// - Parameters:
    ///   - outcomes: the owner's numbers; its page is offered only when the node lets the owner read them.
    ///   - onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(costs: CostsStore, evaluation: EvaluationStore, outcomes: OutcomesStore, onSignInAgain: @escaping () -> Void) {
        self.costs = costs
        self.evaluation = evaluation
        self.outcomes = outcomes
        self.onSignInAgain = onSignInAgain
    }

    /// The pages on offer: the outcomes only once the node has let the owner read them, with nothing said otherwise.
    private var pages: [Page] { Self.pages(isOutcomesAvailable: outcomes.isAvailable) }
    private var shownPage: Page { Self.shown(page, isOutcomesAvailable: outcomes.isAvailable) }

    static func pages(isOutcomesAvailable: Bool) -> [Page] { isOutcomesAvailable ? Page.allCases : [.costs, .evaluation] }

    /// The page to draw, and the one the picker shows selected: the chosen one when it is on offer, else the costs. The picker
    /// reads this too, so that it never holds a selection that is not one of its pages.
    static func shown(_ chosen: Page, isOutcomesAvailable: Bool) -> Page {
        pages(isOutcomesAvailable: isOutcomesAvailable).contains(chosen) ? chosen : .costs
    }

    public var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker(Ja.Tabs.results, selection: Binding(get: { shownPage }, set: { page = $0 })) {
                    ForEach(pages, id: \.self) { Text($0.title).tag($0) }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, Theme.gutter)
                .padding(.vertical, 8)
                switch shownPage {
                case .costs: CostsPage(store: costs, onSignInAgain: onSignInAgain)
                case .evaluation: EvaluationPage(store: evaluation, onSignInAgain: onSignInAgain)
                case .outcomes: OutcomesPage(store: outcomes, onSignInAgain: onSignInAgain)
                }
            }
            .navigationTitle(Ja.Tabs.results)
            // Whether the page is offered is known only after the node has answered the owner.
            .task { await outcomes.refresh() }
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
        }
    }
}
#endif
