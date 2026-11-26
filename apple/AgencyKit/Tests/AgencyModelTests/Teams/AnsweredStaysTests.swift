@testable import AgencyAPI
@testable import AgencyModel
import Foundation
import Testing

/// One source that answers every screen's reads, with an account that has nothing in it (no orgs): what the screens must keep
/// saying, as the owner's rule says (agency#303, #314), through a refresh under way, a failed one and a cancelled one.
private final class EmptyAccountSource: OverviewSource, OrgChartSource, EvaluationSource, @unchecked Sendable {
    enum Mode { case answer, fail, cancel, hold }

    private let lock = NSLock()
    private var mode = Mode.answer
    private var gate: CheckedContinuation<Void, Never>?

    func set(_ mode: Mode) { lock.withLock { self.mode = mode } }
    var isHolding: Bool { lock.withLock { gate != nil } }
    func release() { lock.withLock { let waiting = gate; gate = nil; return waiting }?.resume() }

    func me() async throws -> Me {
        switch lock.withLock({ mode }) {
        case .answer: break
        case .fail: throw NodeError.http(status: 503, message: "down")
        case .cancel: throw CancellationError()
        case .hold: await withCheckedContinuation { continuation in lock.withLock { gate = continuation } }
        }
        return try JSONDecoder().decode(Me.self, from: Data(#"{"account":{"fingerprint":"x","id":"alice","label":"A"},"funding":[],"orgs":[]}"#.utf8))
    }

    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot { throw NodeError.http(status: 404, message: nil) }
    func usage(org: String, team: String, days: Int) async throws -> TokenUsage { throw NodeError.http(status: 404, message: nil) }
    func orgDefinition(org: String) async throws -> OrgDefinition { throw NodeError.http(status: 404, message: nil) }
    func evaluation(org: String, days: Int) async throws -> Evaluation { throw NodeError.http(status: 404, message: nil) }
    func effort(org: String, days: Int) async throws -> Effort { throw NodeError.http(status: 404, message: nil) }
    func orgPlans(org: String) async throws -> OrgPlans { throw NodeError.http(status: 404, message: nil) }
}

/// What a screen reads of a store: whether it has answered, and what it is doing.
@MainActor
private struct Screen {
    let name: String
    let hasAnswered: () -> Bool
    let isLoading: () -> Bool
    /// The state of the read as text, to see that a cancelled read leaves it as it was.
    let state: () -> String
    let refresh: () async -> Void
}

@MainActor
@Suite("A screen that has answered stays answered")
struct AnsweredStaysTests {
    private func screens(_ source: EmptyAccountSource) -> [Screen] {
        let overview = OverviewStore(source: source, retry: .never)
        let costs = CostsStore(source: source, retry: .never)
        let orgChart = OrgChartStore(source: source, retry: .never)
        let people = PeopleStore(source: source, retry: .never)
        let evaluation = EvaluationStore(source: source, retry: .never)
        return [
            Screen(name: "overview", hasAnswered: { overview.hasAnswered }, isLoading: { overview.loading == .loading }, state: { "\(overview.loading)" }, refresh: { await overview.refresh() }),
            Screen(name: "costs", hasAnswered: { costs.hasAnswered }, isLoading: { costs.loading == .loading }, state: { "\(costs.loading)" }, refresh: { await costs.refresh() }),
            Screen(name: "org chart", hasAnswered: { orgChart.hasAnswered }, isLoading: { orgChart.loading == .loading }, state: { "\(orgChart.loading)" }, refresh: { await orgChart.refresh() }),
            Screen(name: "people", hasAnswered: { people.hasAnswered }, isLoading: { people.loading == .loading }, state: { "\(people.loading)" }, refresh: { await people.refresh() }),
            Screen(name: "evaluation", hasAnswered: { evaluation.hasAnswered }, isLoading: { evaluation.loading == .loading }, state: { "\(evaluation.loading)" }, refresh: { await evaluation.refresh() }),
        ]
    }

    @Test("before any answer nothing has answered, a first read of an account with nothing in it answers, and a failed first read does not")
    func firstRead() async {
        let source = EmptyAccountSource()
        for screen in screens(source) { #expect(!screen.hasAnswered(), Comment(rawValue: screen.name)) }
        source.set(.fail)
        for screen in screens(source) {
            await screen.refresh()
            #expect(!screen.hasAnswered(), Comment(rawValue: "\(screen.name): a failed first read is no answer"))
        }
        source.set(.answer)
        for screen in screens(source) {
            await screen.refresh()
            #expect(screen.hasAnswered(), Comment(rawValue: "\(screen.name): an empty account has answered"))
        }
    }

    @Test("once answered, a refresh under way, a failed refresh and a cancelled one leave the screen answered")
    func stays() async {
        let source = EmptyAccountSource()
        for screen in screens(source) {
            source.set(.answer)
            await screen.refresh()
            #expect(screen.hasAnswered())

            source.set(.hold)
            let reading = Task { await screen.refresh() }
            while !source.isHolding { await Task.yield() }
            #expect(screen.isLoading(), Comment(rawValue: "\(screen.name): under way"))
            #expect(screen.hasAnswered(), Comment(rawValue: "\(screen.name): a refresh under way is not unread"))
            source.set(.answer)
            source.release()
            await reading.value

            source.set(.fail)
            await screen.refresh()
            #expect(screen.hasAnswered(), Comment(rawValue: "\(screen.name): a failed refresh"))

            let before = screen.state()
            source.set(.cancel)
            await screen.refresh()
            #expect(screen.hasAnswered(), Comment(rawValue: "\(screen.name): a cancelled refresh"))
            #expect(screen.state() == before, Comment(rawValue: "\(screen.name): a cancelled read leaves the state as it was"))
            #expect(!screen.isLoading(), Comment(rawValue: "\(screen.name): not left loading"))
        }
    }

    @Test("a first read that is cancelled has not answered")
    func cancelledFirst() async {
        let source = EmptyAccountSource()
        source.set(.cancel)
        for screen in screens(source) {
            await screen.refresh()
            #expect(!screen.hasAnswered(), Comment(rawValue: screen.name))
        }
    }
}
