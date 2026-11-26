import Foundation
import Testing

/// A screen must not show a value the node did not give (agency#303): a `0`, an empty list or a default that stands in
/// for the answer makes "not read yet" look like "really nothing". This reads the sources of the screens, the widgets and
/// the app for the ways such a value gets in, and every use must be named here with why it is not one.
@Suite("No stand-in for a value of the node")
struct NoStandInValuesTests {
    private struct Use: Hashable {
        let file: String
        let line: String
    }

    /// `x ?? 0`, `?? []`, `?? ""`, `?? false`, `?? true`, `?? [:]`: a default for a missing value.
    private static let defaultedValue = try! NSRegularExpression(pattern: #"\?\?\s*(0(\.0)?|\[\]|""|false|true|\[:\])(?![\w.])"#)
    /// `private(set) var x = true` or a number: state that starts as a claim before anything was read.
    private static let startingClaim = try! NSRegularExpression(pattern: #"private\(set\) var \w+(: \w+)? = (true|\d+)\s*$"#)

    private static let viewLayers = ["AgencyKit/Sources/AgencyUI", "AgencyKit/Sources/AgencyWidget", "Shared", "App", "Mac", "Widgets"]
    private static let stateLayers = viewLayers + ["AgencyKit/Sources/AgencyModel"]

    /// What is allowed, and why. Anything new fails until it is read and named here.
    private static let allowedDefaults: [Use: String] = [
        Use(file: "AgencyKit/Sources/AgencyUI/Costs/StackedColumns.swift", line: "private var maximum: Double { max(columns.map(\\.total).max() ?? 0, 0.01) }"):
            "the scale of a chart that has no columns",
        Use(file: "AgencyKit/Sources/AgencyUI/Charts/ChartPlots.swift", line: "private var dayCount: Int { lines.first?.count ?? 0 }"):
            "the width of a plot with no lines",
        Use(file: "AgencyKit/Sources/AgencyUI/Evaluation/EvaluationChartView.swift", line: "ColumnPlot(values: chart.days.map { $0.values.map { $0 ?? 0 } }, colors: colors, maximum: chart.max, selected: $selected)"):
            "a day with no value in a series draws no column: the chart only exists after the answer",
        Use(file: "AgencyKit/Sources/AgencyUI/Evaluation/EvaluationChartView.swift", line: "if chart.kind == .lines || (day.values[index] ?? 0) > 0 {"):
            "the same: a day without a value is not listed in the legend of the open day",
        Use(file: "AgencyKit/Sources/AgencyUI/OrgChart/OrgChartScreen.swift", line: "onOpenTeam: { node in (node.signals?.mine ?? 0) > 0 ? onOpenDesk() : onOpenOverview() },"):
            "where a tap goes, not a shown value: a team without signals has nothing of the owner's to open",
        Use(file: "AgencyKit/Sources/AgencyUI/OrgChart/OrgChartRowView.swift", line: "if node.signals?.mine ?? 0 > 0 { Rectangle().fill(Color.accentColor).frame(width: 4).clipShape(RoundedRectangle(cornerRadius: 2)).padding(.vertical, 1) }"):
            "a bar is drawn only for a team that has signals saying something waits; none shows nothing, and the box says the team is loading or unavailable",
        Use(file: "AgencyKit/Sources/AgencyUI/Evaluation/EvaluationChartView.swift", line: "Text(day.shortDate + (day.totalText.map { \"  \" + $0 } ?? \"\")).font(.footnote.weight(.semibold))"):
            "text added after the date of an open day when the series has a total; the chart only exists after the answer",
        Use(file: "Shared/Native/NativeSession.swift", line: "let ownKey = signer?.publicKey.string ?? \"\""):
            "this device has no key: nothing of the node's",
        Use(file: "Shared/Access/AccessSignInFlow.swift", line: "Bundle.main.object(forInfoDictionaryKey: \"AgencyAssociatedHost\") as? String ?? \"\""):
            "a build setting of this app; empty means the sign-in cannot start, which it says",
        Use(file: "Shared/Native/DraftDefaultsStorage.swift", line: "UserDefaults.standard.data(forKey: key).flatMap { try? JSONDecoder().decode([String: DeskDraft].self, from: $0) } ?? [:]"):
            "the owner's own drafts kept on this device, not the node's",
        Use(file: "AgencyKit/Sources/AgencyWidget/WidgetViews.swift", line: "private var count: Int { snapshot?.waitingCount ?? 0 }"):
            "used only after `snapshot == nil` is told apart: the count shows a dash and the lines the name until a snapshot is written",
        Use(file: "AgencyKit/Sources/AgencyUI/Desk/DeskScreen.swift", line: "store: store, isDeviceReady: pairing?.isReady ?? false, isDeviceChecked: pairing.map { $0.status != .checking } ?? true,"):
            "no pairing store means this device has no key, which is known; while the check runs the screen says nothing of it",
        Use(file: "Shared/Native/NativeSession.swift", line: "NotificationOwn(deviceKey: ownKey, canApprove: pairing?.isReady ?? false, sentDecisionIds: sent.ids)"):
            "what the device may approve decides which banners are made, not a shown value",
        Use(file: "Shared/Push/PushCoordinator.swift", line: "let isGranted = (try? await center.requestAuthorization(options: [.alert, .sound, .badge])) ?? false"):
            "the system's answer to the permission prompt, not the node's",
    ]

    private static let allowedClaims: [Use: String] = [
        Use(file: "AgencyKit/Sources/AgencyModel/Costs/CostsStore.swift", line: "public private(set) var days = 14"):
            "the period the owner picks, not a value of the node",
    ]

    @Test("a default for a missing value in a screen, a widget or the app is named and reasoned, or it fails")
    func defaults() throws {
        let found = try Self.uses(of: Self.defaultedValue, in: Self.viewLayers)
        #expect(Set(found.keys) == Set(Self.allowedDefaults.keys), Comment(rawValue: Self.difference(found, Self.allowedDefaults)))
    }

    @Test("state that starts as a claim before anything was read is named and reasoned, or it fails")
    func claims() throws {
        let found = try Self.uses(of: Self.startingClaim, in: Self.stateLayers)
        #expect(Set(found.keys) == Set(Self.allowedClaims.keys), Comment(rawValue: Self.difference(found, Self.allowedClaims)))
    }

    @Test("the scan sees a stand-in when there is one")
    func seesOne() {
        for text in ["let n = store.count ?? 0", "x ?? []", #"label ?? """#, "flag ?? true", "map ?? [:]"] {
            #expect(Self.defaultedValue.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil, Comment(rawValue: text))
        }
        for text in ["a ?? 0.5", "a ?? other", "a ?? 0x10", "a ?? .zero"] {
            #expect(Self.defaultedValue.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) == nil, Comment(rawValue: text))
        }
        let claim = "    public private(set) var nodeSends = true"
        #expect(Self.startingClaim.firstMatch(in: claim, range: NSRange(claim.startIndex..., in: claim)) != nil)
    }

    // MARK: Reading the sources

    private static func root(from file: String = #filePath) -> URL {
        // Tests/AgencyKitTests/<file>: four levels up is `apple/`.
        var url = URL(fileURLWithPath: file)
        for _ in 0..<4 { url.deleteLastPathComponent() }
        return url
    }

    private static func uses(of pattern: NSRegularExpression, in directories: [String]) throws -> [Use: Int] {
        var found: [Use: Int] = [:]
        let root = root()
        for directory in directories {
            let base = root.appendingPathComponent(directory)
            guard let files = FileManager.default.enumerator(at: base, includingPropertiesForKeys: nil) else { continue }
            for case let url as URL in files where url.pathExtension == "swift" {
                let relative = String(url.path.dropFirst(root.path.count + 1))
                let text = try String(contentsOf: url, encoding: .utf8)
                for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
                    let trimmed = line.trimmingCharacters(in: .whitespaces)
                    guard !trimmed.hasPrefix("//") else { continue }
                    if pattern.firstMatch(in: trimmed, range: NSRange(trimmed.startIndex..., in: trimmed)) != nil {
                        found[Use(file: relative, line: trimmed), default: 0] += 1
                    }
                }
            }
        }
        return found
    }

    private static func difference(_ found: [Use: Int], _ allowed: [Use: String]) -> String {
        let unnamed = found.keys.filter { allowed[$0] == nil }.map { "\($0.file): \($0.line)" }.sorted()
        let stale = allowed.keys.filter { found[$0] == nil }.map { "\($0.file): \($0.line)" }.sorted()
        return "not named: \(unnamed); named but gone: \(stale)"
    }
}
