#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import SwiftUI
import Testing
@testable import AgencyUI

/// A team's weekly budget as the node says it, drawn at phone width in light and dark: nothing is wider than the screen. With
/// `AGENCY_SNAPSHOT_DIR` set, each test leaves a PNG to look at.
@MainActor
@Suite("Weekly budget renderings", .serialized)
struct BudgetSnapshotTests {
    private func status(_ team: BudgetPart, org: BudgetPart = BudgetPart(capUsd: 500, spentUsd: 100, state: .ok)) -> BudgetRead {
        .reported(BudgetReport(BudgetStatus(weekStart: "2026-11-23", team: team, org: org)))
    }

    private func expectFits(_ result: Snapshot.Result, _ name: String, sourceLocation: SourceLocation = #_sourceLocation) {
        #expect(!result.png.isEmpty, "\(name) was not drawn", sourceLocation: sourceLocation)
        #expect(result.size.width == Snapshot.phoneWidth, "\(name) is \(result.size.width) points wide", sourceLocation: sourceLocation)
        #expect(result.size.height > 20, sourceLocation: sourceLocation)
    }

    @Test("each state of a team, the team's usage not read, and the org's budget, light and dark")
    func states() {
        let reads: [(String, BudgetRead)] = [
            ("ok", status(BudgetPart(capUsd: 200, spentUsd: 40, state: .ok))),
            ("warning", status(BudgetPart(capUsd: 200, spentUsd: 163.2, state: .warning), org: BudgetPart(capUsd: 500, spentUsd: 311, state: .ok, isPartial: true))),
            ("over", status(BudgetPart(capUsd: 200, spentUsd: 260, state: .over), org: BudgetPart(capUsd: 500, spentUsd: 600, state: .over, isPartial: true))),
            ("team-partial", status(BudgetPart(capUsd: 200, spentUsd: 0, state: .ok, isPartial: true), org: BudgetPart(capUsd: 500, spentUsd: 0, state: .ok, isPartial: true))),
            ("no-cap", status(BudgetPart(capUsd: nil, spentUsd: 5, state: .over), org: BudgetPart(capUsd: 500, spentUsd: 600, state: .over))),
            ("unknown-state", status(BudgetPart(capUsd: 200, spentUsd: 40, state: .unknown))),
        ]
        for scheme in [ColorScheme.light, .dark] {
            for (name, read) in reads {
                expectFits(Snapshot.capture("budget-\(name)", WeeklyBudgetView(read: read).padding(Theme.gutter), scheme: scheme), "\(name) (\(scheme))")
            }
        }
    }

    @Test("what a node that does not report the week says, the failure with a long reason, and a read under way")
    func others() {
        let reads: [(String, BudgetRead)] = [
            ("plain", .plain(Ja.Effort.budgetPlain("$140"))),
            ("failed", .failed("connection refused \(UIFixtures.unbrokenID) and what the system said after it, at some length")),
            ("loading", .loading),
        ]
        for scheme in [ColorScheme.light, .dark] {
            for (name, read) in reads {
                expectFits(Snapshot.capture("budget-\(name)", WeeklyBudgetView(read: read).padding(Theme.gutter), scheme: scheme), "\(name) (\(scheme))")
            }
        }
    }

    @Test("a team with nothing to say draws nothing at all")
    func nothing() {
        let empty = BudgetReport(BudgetStatus(
            weekStart: "2026-11-23", team: BudgetPart(capUsd: nil, spentUsd: 1, state: .ok), org: BudgetPart(capUsd: 500, spentUsd: 1, state: .ok)
        ))
        #expect(empty.isEmpty)
    }
}
#endif
