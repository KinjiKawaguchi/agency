import AgencyKit
import AgencyWidget
import SwiftUI
import WidgetKit

/// What a widget draws at one moment: the file the app wrote, as it is.
struct DeskEntry: TimelineEntry {
    let date: Date
    let snapshot: WidgetSnapshot?
}

/// Reads the snapshot the app wrote into the app group. The widget never holds the Access token or a device key,
/// and never asks the node for anything: this file is all it has.
struct DeskProvider: TimelineProvider {
    /// How often the widget asks the system for a reload. The system budgets reloads, so it can be later.
    private static let refresh: TimeInterval = 30 * 60

    private func snapshot() -> WidgetSnapshot? {
        AppGroup.identifier.flatMap(WidgetSnapshotStore.appGroup)?.read()
    }

    func placeholder(in context: Context) -> DeskEntry {
        DeskEntry(date: Date(), snapshot: Self.sample)
    }

    func getSnapshot(in context: Context, completion: @escaping (DeskEntry) -> Void) {
        completion(DeskEntry(date: Date(), snapshot: context.isPreview ? Self.sample : snapshot()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<DeskEntry>) -> Void) {
        let now = Date()
        completion(Timeline(entries: [DeskEntry(date: now, snapshot: snapshot())], policy: .after(now.addingTimeInterval(Self.refresh))))
    }

    /// What the gallery shows: made up, never a real request.
    private static var sample: WidgetSnapshot {
        WidgetSnapshot(host: "node.example", writtenAt: Date(), waitingCount: 3, requests: [
            WidgetRequest(org: "acme", team: "app", request: "r-1", kind: .proposal, title: "クラウド移行計画の承認", teamLabel: "App"),
            WidgetRequest(org: "acme", team: "app", request: "r-2", kind: .question, title: "どのデータベースを使いますか?", teamLabel: "App"),
            WidgetRequest(org: "acme", team: "site", request: "r-3", kind: .pendingDecision, title: "旧形式の廃止を決める", teamLabel: "Site"),
        ])
    }
}

/// The app group the file lives in, from the app's own `Info.plist`: the app's bundle ID with `group.` in front.
enum AppGroup {
    static var identifier: String? {
        Bundle.main.object(forInfoDictionaryKey: "AgencyAppGroup") as? String
    }
}

private struct DeskWidgetView: View {
    let entry: DeskEntry
    @Environment(\.widgetFamily) private var family

    private var layout: WidgetLayout {
        switch family {
        case .systemMedium: .medium
        case .accessoryCircular: .circular
        case .accessoryRectangular: .rectangular
        case .accessoryInline: .inline
        default: .small
        }
    }

    var body: some View {
        WidgetContent(snapshot: entry.snapshot, layout: layout, now: entry.date)
            .widgetURL(WidgetContent.link(for: entry.snapshot))
            .containerBackground(.fill.tertiary, for: .widget)
    }
}

/// "What waits on you": the number of requests that are the viewer's to decide, and the first few.
struct DeskWidget: Widget {
    let kind = "AgencyDesk"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: DeskProvider()) { entry in
            DeskWidgetView(entry: entry)
        }
        .configurationDisplayName(WidgetWording.displayName)
        .description(WidgetWording.description)
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryCircular, .accessoryRectangular, .accessoryInline])
    }
}
