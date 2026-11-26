#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// The evaluation of one org: what it is, the choice of org and period, then each team. The content only: the
/// screen around it adds the title, pull to reload and the banners.
struct EvaluationContent: View {
    let orgs: [OrgRef]
    let org: OrgRef?
    let days: Int
    let state: EvaluationState
    var effort: EffortState = .loading
    var onChooseOrg: ((String) -> Void)?
    var onChooseDays: ((Int) -> Void)?
    var onTryAgain: (() -> Void)?
    var selectedDay: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            Text(Ja.Evaluation.lead).font(.subheadline).foregroundStyle(.secondary).wrapping()
            if orgs.isEmpty {
                Text(Ja.Evaluation.noOrgs).font(.subheadline).foregroundStyle(.secondary).wrapping()
            } else {
                controls
                stateView
                EffortSection(state: effort, onTryAgain: onTryAgain, selectedDay: selectedDay).padding(.top, 20)
            }
        }
    }

    private var controls: some View {
        VStack(alignment: .leading, spacing: 8) {
            if orgs.count > 1 {
                Text(Ja.Evaluation.org).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                FlowLine(spacing: 8) {
                    ForEach(orgs, id: \.id) { one in
                        Button(one.label) { onChooseOrg?(one.id) }
                            .buttonStyle(ChoiceStyle(isSelected: one.id == org?.id))
                    }
                }
            }
            Text(Ja.Evaluation.period).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            HStack(spacing: 8) {
                ForEach(EvaluationStore.periods, id: \.self) { period in
                    Button(Ja.Evaluation.days(period)) { onChooseDays?(period) }
                        .buttonStyle(ChoiceStyle(isSelected: period == days))
                }
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var stateView: some View {
        switch state {
        case .loading:
            SkeletonCard()
        case .unavailable:
            Text(Ja.Evaluation.unavailable).font(.subheadline).foregroundStyle(.secondary).wrapping()
        case .couldNotAnswer(let reason), .failed(let reason):
            FailedRegion(what: Ja.Evaluation.title, why: reason, onTryAgain: onTryAgain)
        case .ready(let reports, let unavailable):
            if reports.isEmpty && unavailable.isEmpty {
                Text(Ja.Evaluation.noTeams).font(.subheadline).foregroundStyle(.secondary).wrapping()
            }
            ForEach(reports) { TeamEvaluationView(report: $0, selectedDay: selectedDay).padding(.top, 12) }
            ForEach(unavailable) { note in
                VStack(alignment: .leading, spacing: 6) {
                    Text(note.title).font(.title3.weight(.semibold)).wrapping()
                    Text(note.text).font(.subheadline).foregroundStyle(.secondary).wrapping()
                }
                .padding(.top, 12)
            }
        }
    }
}
#endif
