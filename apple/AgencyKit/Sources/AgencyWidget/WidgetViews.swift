#if canImport(SwiftUI)
import AgencyKit
import SwiftUI

/// The shapes a widget takes, without WidgetKit's names, so the views draw (and are tested) as plain SwiftUI.
public enum WidgetLayout: Sendable, Equatable {
    case small, medium, circular, rectangular, inline
}

/// What a widget draws: the number of requests that wait on the viewer and the first few of them.
/// Titles are privacy sensitive: on the lock screen the system hides them and leaves the count.
public struct WidgetContent: View {
    private let snapshot: WidgetSnapshot?
    private let layout: WidgetLayout
    private let now: Date
    private let showsLinks: Bool

    /// - Parameters:
    ///   - snapshot: what the app wrote; `nil` before the app has run.
    ///   - showsLinks: each request of the medium widget opens itself. Off for a rendering, which cannot draw a link.
    public init(snapshot: WidgetSnapshot?, layout: WidgetLayout, now: Date = Date(), showsLinks: Bool = true) {
        self.snapshot = snapshot
        self.layout = layout
        self.now = now
        self.showsLinks = showsLinks
    }

    /// The link of the widget as a whole: the first request, when there is one.
    public static func link(for snapshot: WidgetSnapshot?) -> URL? {
        snapshot?.requests.first.flatMap { RequestLink(org: $0.org, team: $0.team, request: $0.request) }.map(WidgetLink.url(for:))
    }

    public var body: some View {
        switch layout {
        case .inline: inline
        case .circular: circular
        case .rectangular: rectangular
        case .small: small
        case .medium: medium
        }
    }

    // MARK: Pieces

    private var count: Int { snapshot?.waitingCount ?? 0 }
    /// A dash until the app has written a snapshot: no count is not a count of nothing.
    private var countText: String { snapshot == nil ? "–" : "\(count)" }

    private var staleNote: String? {
        guard let snapshot, case .stale(let minutes) = snapshot.staleness(now: now) else { return nil }
        return WidgetWording.updated(minutes: minutes)
    }

    /// The request's title, or its kind when titles are hidden.
    private func title(_ request: WidgetRequest, lines: Int) -> some View {
        Text(request.title ?? WidgetWording.kind(request.kind)).font(.footnote.weight(.semibold)).lineLimit(lines).multilineTextAlignment(.leading).privacySensitive()
    }

    private var emptyText: String { snapshot == nil ? WidgetWording.notSynced : WidgetWording.nothingWaits }

    // MARK: Lock screen

    private var inline: some View {
        Text(snapshot == nil ? WidgetWording.displayName : WidgetWording.inline(count))
    }

    private var circular: some View {
        VStack(spacing: 0) {
            Text(countText).font(.title2.weight(.bold).monospacedDigit()).minimumScaleFactor(0.6)
            Text(WidgetWording.displayName).font(.system(size: 9)).lineLimit(1).minimumScaleFactor(0.6)
        }
    }

    private var rectangular: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(snapshot == nil ? WidgetWording.displayName : WidgetWording.inline(count)).font(.footnote.weight(.bold))
            if let first = snapshot?.requests.first {
                title(first, lines: 2)
            } else {
                Text(emptyText).font(.caption2)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: Home screen

    private var small: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(WidgetWording.displayName).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            HStack(alignment: .firstTextBaseline, spacing: 2) {
                Text(countText).font(.system(size: 40, weight: .bold, design: .rounded).monospacedDigit())
                Text("件").font(.footnote).foregroundStyle(.secondary)
            }
            if let first = snapshot?.requests.first {
                title(first, lines: 2)
                if let team = first.teamLabel { Text(team).font(.caption2).foregroundStyle(.secondary).lineLimit(1) }
            } else {
                Text(emptyText).font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
            if let staleNote { Text(staleNote).font(.system(size: 9)).foregroundStyle(.secondary).lineLimit(1) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var medium: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(WidgetWording.displayName).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Text(countText).font(.system(size: 44, weight: .bold, design: .rounded).monospacedDigit())
                Text(WidgetWording.waitingLabel).font(.caption2).foregroundStyle(.secondary).lineLimit(2)
                Spacer(minLength: 0)
                if let staleNote { Text(staleNote).font(.system(size: 9)).foregroundStyle(.secondary).lineLimit(2) }
            }
            .frame(width: 96, alignment: .leading)
            VStack(alignment: .leading, spacing: 6) {
                if let requests = snapshot?.requests, !requests.isEmpty {
                    ForEach(Array(requests.enumerated()), id: \.offset) { _, request in row(request) }
                    if count > requests.count {
                        Text(WidgetWording.more(count - requests.count)).font(.caption2).foregroundStyle(.secondary)
                    }
                } else {
                    Text(emptyText).font(.footnote).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder
    private func row(_ request: WidgetRequest) -> some View {
        let content = VStack(alignment: .leading, spacing: 0) {
            title(request, lines: 1)
            if let team = request.teamLabel {
                Text("\(team) · \(WidgetWording.kind(request.kind))").font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        if showsLinks, let link = RequestLink(org: request.org, team: request.team, request: request.request) {
            Link(destination: WidgetLink.url(for: link)) { content }
        } else {
            content
        }
    }
}
#endif
