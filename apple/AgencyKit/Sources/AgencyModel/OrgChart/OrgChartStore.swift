import AgencyAPI
import Foundation
import Observation

/// Where the org chart reads from: the home node's Client API. `NodeClient` is the real one.
public protocol OrgChartSource: TeamSnapshotSource {
    func orgDefinition(org: String) async throws -> OrgDefinition
}

extension NodeClient: OrgChartSource {}

/// The org chart: every org of the account side by side, or one org, with the overview's signals on each team.
@MainActor
@Observable
public final class OrgChartStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    /// The value of the choice that draws every org in one chart.
    public static let all = "all"

    public private(set) var loading: Loading = .idle
    /// Whether a read has finished at least once. Set once and never reset: a refresh under way, a failed one or a cancelled one
    /// does not make the screen unread again, so an account with nothing in it keeps saying so instead of going back to bars.
    public private(set) var hasAnswered = false
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var account: AccountRef?
    public private(set) var orgs: [OrgRef] = []
    /// What was chosen: `all`, or an org's id. Several orgs and no choice draws every one.
    public private(set) var selection: String?
    /// The department the one-org chart is focused on.
    public private(set) var focus: String?

    @ObservationIgnored private let source: any OrgChartSource
    @ObservationIgnored private let retry: RetryPolicy
    @ObservationIgnored private let flight = SingleFlight()
    private var definitions: [String: OrgDefinition] = [:]
    private var definitionFailed: Set<String> = []
    private var teams: [String: Team] = [:]
    /// The `org/team` keys of the teams that are still being read.
    private var teamsLoading: Set<String> = []

    public init(source: any OrgChartSource, retry: RetryPolicy = .standard) {
        self.source = source
        self.retry = retry
    }

    // MARK: What the screen shows

    /// Every org is drawn when there are several and none is named; otherwise one org.
    public var isAll: Bool { orgs.count > 1 && (selection == nil || selection == Self.all) }

    /// The org drawn when one is: the one chosen, or the first.
    public var org: OrgRef? { orgs.first { $0.id == selection } ?? orgs.first }

    public var title: String {
        isAll ? Ja.OrgChart.allTitle : org.map { Ja.OrgChart.orgTitle($0.label) } ?? Ja.OrgChart.title
    }

    private var inputs: [OrgInput] {
        orgs.map { OrgInput(org: $0, definition: definitions[$0.id], failed: definitionFailed.contains($0.id)) }
    }

    /// The chart of the one org drawn, when its definition has been read.
    private var tree: OneOrgTree? {
        guard let account, let org, let definition = definitions[org.id] else { return nil }
        return OrgTreeBuilder.oneOrg(definition, teams: Array(teams.values), viewerAccount: account.id, focus: focus, loading: teamsLoading)
    }

    /// The tree as a list of lines, each with how deep it sits.
    public var rows: [OrgChartRow] {
        guard let account else { return [] }
        if isAll {
            let drawn = OrgTreeBuilder.allOrgs(inputs, teams: Array(teams.values), viewerAccount: account.id, loading: teamsLoading)
            return OrgChartRow.allOrgs(account: account.label, orgs: drawn)
        }
        guard let org, let tree else { return [] }
        return OrgChartRow.oneOrg(label: org.label, orgID: org.id, tree: tree)
    }

    /// The one org's chart is focused on a department and it has nobody in it.
    public var isEmptyDepartment: Bool { tree.map { $0.focus != nil && $0.children.isEmpty } ?? false }

    /// The org's definition could not be read, and none was read before: the chart of one org has nothing to draw.
    public var isOrgUnavailable: Bool { !isAll && org.map { definitionFailed.contains($0.id) && definitions[$0.id] == nil } == true }

    /// Whether the one org's chart is still waiting for its definition.
    public var isOrgLoading: Bool { !isAll && org.map { definitions[$0.id] == nil && !definitionFailed.contains($0.id) } == true }

    // MARK: Choosing

    public func choose(_ value: String) {
        guard value == Self.all ? orgs.count > 1 : orgs.contains(where: { $0.id == value }) else { return }
        selection = value
        focus = nil
    }

    /// Focuses the chart of one org on a department and what is below it; `nil` goes back to the whole org.
    public func focus(on department: String?) {
        focus = department
    }

    // MARK: Reading

    /// Reads again. A read already running is waited for, not repeated.
    public func refresh() async {
        await flight.run { await self.reload() }
    }

    private func reload() async {
        let before = loading
        loading = .loading
        do {
            let me = try await source.me()
            account = me.account
            orgs = me.orgs
            if let selection, selection != Self.all, !me.orgs.contains(where: { $0.id == selection }) { self.selection = nil }
            needsSignIn = false
            let keys = Set(me.orgs.flatMap { org in org.teams.map { TeamReader.key(org: org.id, team: $0.id) } })
            teams = teams.filter { keys.contains($0.key) }
            definitions = definitions.filter { id, _ in me.orgs.contains { $0.id == id } }
            teamsLoading = keys.subtracting(teams.keys)
            async let definitionsRead: Void = readDefinitions(of: me.orgs)
            await readTeams(of: me.orgs)
            await definitionsRead
            hasAnswered = true
            loading = .loaded
        } catch let error as NodeError where error == .sessionExpired || error == .notSignedIn {
            needsSignIn = true
            loading = .failed(Ja.Desk.signInEnded)
        } catch where DeskStore.isCancelled(error) {
            // A read that was cancelled says nothing: what was there stays.
            loading = before
        } catch {
            loading = .failed(DeskStore.reason(error))
        }
    }

    private func readTeams(of orgs: [OrgRef]) async {
        await TeamReader.read(orgs, from: source, retry: retry) { key, result in
            teamsLoading.remove(key)
            switch result {
            case .success(let snapshot):
                teams[key] = Team(snapshot: snapshot)
            case .failure(let error):
                // A team that was read keeps its last answer until it can be read again.
                if TeamFailure(error: error).signInEnded { needsSignIn = true }
            }
        }
    }

    private func readDefinitions(of orgs: [OrgRef]) async {
        let (source, retry) = (source, retry)
        await withTaskGroup(of: (String, Result<OrgDefinition, Error>).self) { group in
            for org in orgs {
                group.addTask {
                    do { return (org.id, .success(try await retry.run { try await source.orgDefinition(org: org.id) })) }
                    catch { return (org.id, .failure(error)) }
                }
            }
            for await (id, result) in group {
                switch result {
                case .success(let definition):
                    definitions[id] = definition
                    definitionFailed.remove(id)
                case .failure(let error):
                    if error is CancellationError { continue }
                    // A definition that was read stays on screen; only an org never read says it cannot be.
                    if definitions[id] == nil { definitionFailed.insert(id) }
                    if TeamFailure(error: error).signInEnded { needsSignIn = true }
                }
            }
        }
    }
}
