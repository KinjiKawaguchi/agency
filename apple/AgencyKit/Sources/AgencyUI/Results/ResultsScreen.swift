#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The results tab: the costs and the evaluation, behind a switch at the top, as the web client groups them.
public struct ResultsScreen: View {
    private let costs: CostsStore
    private let evaluation: EvaluationStore
    private let onSignInAgain: () -> Void
    @State private var page: Page = .costs

    enum Page: CaseIterable, Hashable {
        case costs, evaluation

        var title: String {
            switch self {
            case .costs: Ja.Results.costs
            case .evaluation: Ja.Results.evaluation
            }
        }
    }

    /// - Parameter onSignInAgain: runs the Access sign-in again, when the session has ended.
    public init(costs: CostsStore, evaluation: EvaluationStore, onSignInAgain: @escaping () -> Void) {
        self.costs = costs
        self.evaluation = evaluation
        self.onSignInAgain = onSignInAgain
    }

    public var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker(Ja.Results.title, selection: $page) {
                    ForEach(Page.allCases, id: \.self) { Text($0.title).tag($0) }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, Theme.gutter)
                .padding(.vertical, 8)
                switch page {
                case .costs: CostsPage(store: costs, onSignInAgain: onSignInAgain)
                case .evaluation: EvaluationPage(store: evaluation, onSignInAgain: onSignInAgain)
                }
            }
            .navigationTitle(Ja.Results.title)
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
        }
    }
}
#endif
