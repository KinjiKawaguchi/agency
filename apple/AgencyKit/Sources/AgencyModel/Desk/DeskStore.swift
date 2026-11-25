import AgencyAPI
import Foundation
import Observation

/// Where the desk reads from: the home node's Client API. `NodeClient` is the real one.
public protocol DeskSource: Sendable {
    func me() async throws -> Me
    func teamSnapshot(org: String, team: String) async throws -> TeamSnapshot
    func decisions(limit: Int) async throws -> DecisionLog
}

extension NodeClient: DeskSource {}

/// Signs a prepared decision with this device's key and sends it. The app's implementation asks for
/// Face ID; tests use a stand-in.
public protocol DeskSender: Sendable {
    func send(_ decision: PreparedDecision) async throws -> DecisionResult
}

/// Where unsent drafts are kept between launches.
public protocol DraftStorage: Sendable {
    func load() -> [String: DeskDraft]
    func save(_ drafts: [String: DeskDraft])
}

/// What was sent for one request, shown until the request leaves the desk.
public struct SentReply: Equatable, Sendable {
    public let isComment: Bool
    public let outcome: DecisionResult.Outcome
    /// Why a decision was queued.
    public let message: String?

    public init(isComment: Bool, outcome: DecisionResult.Outcome, message: String?) {
        self.isComment = isComment
        self.outcome = outcome
        self.message = message
    }
}

/// The desk: the pile, the owner's drafts, and what happens when one is sent.
@MainActor
@Observable
public final class DeskStore {
    public enum Loading: Equatable, Sendable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    public private(set) var items: [DeskItem] = []
    public private(set) var teams: [Team] = []
    public private(set) var recent: [DecisionEntry] = []
    public private(set) var recentFailure: String?
    public private(set) var loading: Loading = .idle
    /// Teams whose view could not be read, by `org/team`, with the reason.
    public private(set) var teamFailures: [String: String] = [:]
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var drafts: [String: DeskDraft]
    public private(set) var sending: Set<String> = []
    public private(set) var sent: [String: SentReply] = [:]
    /// Why the last send of an item failed. The draft is kept.
    public private(set) var sendFailures: [String: String] = [:]

    @ObservationIgnored private let source: any DeskSource
    @ObservationIgnored private let sender: any DeskSender
    @ObservationIgnored private let storage: any DraftStorage

    public init(source: any DeskSource, sender: any DeskSender, storage: any DraftStorage) {
        self.source = source
        self.sender = sender
        self.storage = storage
        drafts = storage.load()
    }

    /// The number for the tab and the app icon.
    public var waitingCount: Int { Desk.waitingCount(teams) }
    public var situation: Situation { Situation(teams: teams, items: items) }

    public func item(_ id: String) -> DeskItem? { items.first { $0.id == id } }

    // MARK: Reading

    /// Reads every team's view and the recent decisions. A team that cannot be read does not hide the others.
    public func refresh() async {
        loading = .loading
        do {
            let me = try await source.me()
            let source = source
            var snapshots: [(key: String, result: Result<TeamSnapshot, Error>)] = []
            await withTaskGroup(of: (String, Result<TeamSnapshot, Error>).self) { group in
                for org in me.orgs {
                    for team in org.teams {
                        group.addTask {
                            do { return ("\(org.id)/\(team.id)", .success(try await source.teamSnapshot(org: org.id, team: team.id))) }
                            catch { return ("\(org.id)/\(team.id)", .failure(error)) }
                        }
                    }
                }
                for await one in group { snapshots.append((key: one.0, result: one.1)) }
            }
            apply(snapshots.sorted { $0.key < $1.key })
            await refreshRecent()
            needsSignIn = false
            loading = .loaded
        } catch NodeError.sessionExpired {
            needsSignIn = true
            loading = .failed("The sign-in has ended.")
        } catch NodeError.notSignedIn {
            needsSignIn = true
            loading = .failed("Not signed in.")
        } catch {
            loading = .failed(Self.reason(error))
        }
    }

    private func apply(_ snapshots: [(key: String, result: Result<TeamSnapshot, Error>)]) {
        var loaded: [Team] = []
        var failures: [String: String] = [:]
        for (key, result) in snapshots {
            switch result {
            case .success(let snapshot): loaded.append(Team(snapshot: snapshot))
            case .failure(let error): failures[key] = Self.reason(error)
            }
        }
        teams = loaded
        teamFailures = failures
        items = Desk.openItems(loaded)
        let present = Set(items.map(\.id))
        sent = sent.filter { present.contains($0.key) }
    }

    private func refreshRecent() async {
        do {
            recent = try await source.decisions(limit: 20).entries
            recentFailure = nil
        } catch {
            recentFailure = Self.reason(error)
        }
    }

    // MARK: Drafts

    public func draft(for id: String) -> DeskDraft { drafts[id] ?? DeskDraft() }

    public func setText(_ text: String, for id: String) {
        update(id) { $0.text = text }
    }

    /// Picks an option, or lets go of it when it is picked again.
    public func toggleOption(_ key: String, for id: String) {
        update(id) { $0.optionKey = $0.optionKey == key ? nil : key }
    }

    private func update(_ id: String, _ change: (inout DeskDraft) -> Void) {
        var draft = drafts[id] ?? DeskDraft()
        change(&draft)
        drafts[id] = draft.isEmpty ? nil : draft
        sendFailures[id] = nil
        storage.save(drafts)
    }

    // MARK: Sending

    /// Sends a reply and keeps the draft when it fails. Nothing is sent twice for one item at a time.
    public func send(_ reply: DeskReply, for id: String) async {
        guard let item = item(id), !sending.contains(id), sent[id] == nil else { return }
        let decision: PreparedDecision
        do { decision = try item.prepare(reply) } catch {
            sendFailures[id] = Self.reason(error)
            return
        }
        sending.insert(id)
        sendFailures[id] = nil
        defer { sending.remove(id) }
        do {
            let result = try await sender.send(decision)
            drafts[id] = nil
            storage.save(drafts)
            sent[id] = SentReply(isComment: decision.isComment, outcome: result.outcome, message: result.message)
            await refresh()
        } catch NodeError.sessionExpired {
            needsSignIn = true
            sendFailures[id] = "The sign-in has ended."
        } catch {
            sendFailures[id] = Self.reason(error)
        }
    }

    // MARK: Errors

    static func reason(_ error: Error) -> String {
        switch error {
        case NodeError.http(_, let message?): message
        case NodeError.http(let status, nil): "HTTP \(status)"
        case NodeError.notSignedIn: "Not signed in."
        case NodeError.sessionExpired: "The sign-in has ended."
        case NodeError.unreadable: "The node's answer was not understood."
        case let error as DeskReplyError: String(describing: error)
        default: error.localizedDescription
        }
    }
}
