#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One line of the org chart, indented by how deep it sits. The tree is the web client's, as the indented list the
/// web draws on a phone.
struct OrgChartRowView: View {
    let row: OrgChartRow
    var onChooseOrg: ((String) -> Void)?
    var onFocus: ((String) -> Void)?
    var onOpenTeam: ((TeamNode) -> Void)?

    /// How far a level is indented, and the deepest level that indents further.
    private static let step: CGFloat = 14
    private static let deepest = 5

    var body: some View {
        content
            .padding(.leading, CGFloat(min(row.depth, Self.deepest)) * Self.step)
            .overlay(alignment: .leading) {
                if row.depth > 0 { Rectangle().fill(Theme.hairline).frame(width: 1).padding(.leading, CGFloat(min(row.depth, Self.deepest) - 1) * Self.step + 6) }
            }
    }

    @ViewBuilder
    private var content: some View {
        switch row.content {
        case .top(let kind, let name, let sub): top(kind, name, sub)
        case .org(let node): org(node)
        case .department(let node): department(node)
        case .team(let node): TeamBox(node: node, onOpen: onOpenTeam)
        case .person(let node): person(node)
        }
    }

    private func box<Body: View>(_ kind: String, _ name: String, @ViewBuilder _ rest: () -> Body) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(kind).font(.caption).foregroundStyle(.secondary)
            ExpandableTitle(text: name)
            rest()
        }
        .card()
    }

    private func top(_ kind: String, _ name: String, _ sub: String?) -> some View {
        box(kind, name) { if let sub { Text(sub).font(.footnote).foregroundStyle(.secondary).wrapping() } }
    }

    @ViewBuilder
    private func waiting(_ count: Int) -> some View {
        if count > 0 { Text(Ja.Overview.waitingForYou(count)).font(.footnote.weight(.semibold)).foregroundStyle(Color.accentColor).wrapping() }
    }

    private func org(_ node: OrgNode) -> some View {
        let content = box(Ja.OrgChart.organization, node.org.label) {
            switch node.state {
            case .failed: Text(Ja.OrgChart.orgUnavailable).font(.footnote).foregroundStyle(Theme.warning).wrapping()
            case .loading: SkeletonLines(lines: 1)
            case .ready: EmptyView()
            }
            waiting(node.mine)
        }
        return Tappable(action: onChooseOrg.map { choose in { choose(node.id) } }) { content }
    }

    private func department(_ node: DepartmentNode) -> some View {
        let content = box(Ja.OrgChart.department, node.department.label) {
            Text(node.head.map { Ja.OrgChart.ledBy($0.label) } ?? Ja.OrgChart.noHead).font(.footnote).foregroundStyle(.secondary).wrapping()
            waiting(node.mine)
        }
        return Tappable(action: onFocus.map { focus in { focus(node.id) } }) { content }
    }

    private func person(_ node: PersonNode) -> some View {
        let notes = [node.isYou ? Ja.OrgChart.you : nil, node.member.role == .answerer ? Ja.OrgChart.answerer : nil].compactMap { $0 }
        return HStack(spacing: Theme.spacing) {
            Face(label: node.member.label, kind: .human, size: .small)
            VStack(alignment: .leading, spacing: 2) {
                ExpandableTitle(text: node.member.label)
                Text(notes.isEmpty ? Ja.OrgChart.member : notes.joined(separator: "、")).font(.footnote).foregroundStyle(.secondary)
            }
        }
        .card(selected: node.isYou)
        .accessibilityElement(children: .combine)
    }
}

/// A box that can be tapped; without an action (a rendering) it is drawn as it is, not as a disabled button.
struct Tappable<Content: View>: View {
    let action: (() -> Void)?
    @ViewBuilder var content: Content

    var body: some View {
        if let action {
            Button(action: action) { content }.buttonStyle(.plain)
        } else {
            content
        }
    }
}

/// A team of the chart: its signals (waiting on the viewer, stuck, working) and its participants as faces.
struct TeamBox: View {
    let node: TeamNode
    var onOpen: ((TeamNode) -> Void)?

    var body: some View {
        let text = TeamBoxText(node: node)
        let content = VStack(alignment: .leading, spacing: 4) {
            Text(Ja.OrgChart.team).font(.caption).foregroundStyle(.secondary)
            ExpandableTitle(text: node.place.label)
            if let old = text.oldData { Tag(text: old, color: Theme.warning) }
            if node.isLoading {
                SkeletonLines(lines: 2)
            } else if let unavailable = text.unavailable {
                Text(unavailable).font(.footnote).foregroundStyle(Theme.warning).wrapping()
            } else {
                if let waiting = text.waiting { Text(waiting).font(.footnote.weight(.semibold)).foregroundStyle(Color.accentColor).wrapping() }
                if let tag = text.tag { Tag(text: tag, color: Theme.warning) }
                if let working = text.working { Text(working).font(.footnote).foregroundStyle(.secondary).wrapping() }
            }
            faces(text)
        }
        .card()
        .overlay(alignment: .leading) {
            if node.signals?.mine ?? 0 > 0 { Rectangle().fill(Color.accentColor).frame(width: 4).clipShape(RoundedRectangle(cornerRadius: 2)).padding(.vertical, 1) }
        }
        return Tappable(action: onOpen.map { open in { open(node) } }) { content }
    }

    @ViewBuilder
    private func faces(_ text: TeamBoxText) -> some View {
        if !text.faces.isEmpty {
            HStack(spacing: 2) {
                ForEach(text.faces) { agent in
                    Face(label: agent.person.label, kind: .agent, presence: agent.presence, size: .small)
                }
                if let more = text.more { Text(more).font(.caption.weight(.semibold)).foregroundStyle(.secondary).padding(.leading, 4) }
            }
            .padding(.top, 2)
        }
    }
}
#endif
