import AgencyAPI
import AgencyProtocol
import Foundation
import Observation

/// Where the desk reads from: the home node's Client API. `NodeClient` is the real one.
public protocol DeskSource: TeamSnapshotSource {
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
    /// The recoveries the node says wait, one for each org: shown above the desk until the node stops saying it. What was
    /// said last stays when a later read fails.
    public private(set) var recoveries: [RecoveryNotice] = []
    /// What waits for the root key's owner to sign, once the node has said; `nil` when nothing waits or before it answered.
    public private(set) var rootChanges: RootChangeNotice?
    /// What the node says the viewer is (a member it hosts, or anyone else), once it has said: it decides which screens are offered. A
    /// store made for the same node and account as one that had been told (after a key reset) starts from what that one knew, until the
    /// node says it again; otherwise it starts as `loading`.
    public private(set) var standing: Standing = .loading
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
    @ObservationIgnored private let afterSend: @Sendable () async -> Void
    @ObservationIgnored private let retry: RetryPolicy
    /// The last view read of each team, kept when a later read of it fails.
    @ObservationIgnored private var lastRead: [String: Team] = [:]

    /// - Parameter afterSend: runs after a decision was sent and before the desk is read again, to drop
    ///   what a shared cache holds from before it.
    /// - Parameter retry: how a team whose view is not ready is asked again, while the others are shown.
    public init(
        source: any DeskSource, sender: any DeskSender, storage: any DraftStorage,
        afterSend: @escaping @Sendable () async -> Void = {}, retry: RetryPolicy = .standard, standing: Standing = .loading
    ) {
        self.standing = standing
        self.source = source
        self.sender = sender
        self.storage = storage
        self.afterSend = afterSend
        self.retry = retry
        drafts = storage.load()
    }

    /// Whether a read has finished at least once. Set once and never reset: a later read under way or a failed one does not
    /// make the desk unread again.
    public private(set) var hasAnswered = false

    /// Whether the last read that finished failed, wholly (the account, the sign-in) or in part (a team that could not be
    /// read, so the number is not the whole desk). Set when a read fails and cleared when one finishes without a
    /// failure; a read under way leaves it as it was, so a place that shows the number (the Mac's menu bar) does not
    /// look current while the next read of a node that is down is still waiting.
    public private(set) var isReadFailing = false

    /// Whether the node has answered: a read finished, or a team's view was read. Before that the lists are empty because
    /// nothing was read, not because nothing is there, and a screen shows that it is loading, not an empty desk.
    public var hasAnswer: Bool { hasAnswered || !teams.isEmpty }

    /// The number for the tab and the app icon. `0` also before the node has answered: use `knownWaitingCount` where
    /// "not read yet" must not look like "nothing waits".
    public var waitingCount: Int { Desk.waitingCount(teams) }

    /// How many requests wait for the owner, or `nil` until a read has finished (then it stays known: a refresh under way
    /// or a failed one keeps the last number).
    public var knownWaitingCount: Int? { hasAnswered ? waitingCount : nil }
    public var situation: Situation { Situation(teams: teams, items: items) }

    public func item(_ id: String) -> DeskItem? { items.first { $0.id == id } }

    // MARK: Reading

    /// Reads every team's view and the recent decisions. A team that cannot be read does not hide the others.
    public func refresh() async {
        let before = loading
        loading = .loading
        do {
            let me = try await source.me()
            recoveries = RecoveryNotice.all(in: me.orgs)
            rootChanges = RootChangeNotice(me: me)
            standing = Standing(me: me)
            let keys = Set(me.orgs.flatMap { org in org.teams.map { "\(org.id)/\($0.id)" } })
            var snapshots: [(key: String, result: Result<TeamSnapshot, Error>)] = []
            // Each team is shown as soon as it answers, so one that is not ready does not hold back the others.
            await TeamReader.read(me.orgs, from: source, retry: retry) { key, result in
                snapshots.append((key: key, result: result))
                apply(snapshots, keys: keys)
            }
            apply(snapshots, keys: keys)
            await refreshRecent()
            needsSignIn = false
            hasAnswered = true
            isReadFailing = !teamFailures.isEmpty
            loading = .loaded
        } catch where Self.isCancelled(error) {
            // The screen that asked went away (the Mac's sidebar cancels the desk's read when another screen is chosen):
            // that says nothing about the node, so what was known stays as it was.
            loading = before
        } catch NodeError.sessionExpired {
            needsSignIn = true
            isReadFailing = true
            loading = .failed("The sign-in has ended.")
        } catch NodeError.notSignedIn {
            needsSignIn = true
            isReadFailing = true
            loading = .failed("Not signed in.")
        } catch {
            isReadFailing = true
            // Nothing earlier to go by: the viewer is taken for anyone else, whose screens show their own failures. A sign-in that has
            // ended (above) says nothing of who is asking, so it leaves the viewer not yet told.
            if standing == .loading { standing = .unknown }
            loading = .failed(Self.reason(error))
        }
    }

    /// A read the screen that asked for it went away from (the Mac's sidebar cancels it when another screen is chosen): it says
    /// nothing about the node.
    static func isCancelled(_ error: Error) -> Bool {
        error is CancellationError || (error as? URLError)?.code == .cancelled
    }

    /// A team that was read keeps its last answer when a later read of it fails, as the web client does, and
    /// its failure is still reported; a team the account no longer lists is dropped.
    private func apply(_ snapshots: [(key: String, result: Result<TeamSnapshot, Error>)], keys: Set<String>) {
        var failures: [String: String] = [:]
        var read = lastRead.filter { keys.contains($0.key) }
        for (key, result) in snapshots {
            switch result {
            case .success(let snapshot): read[key] = Team(snapshot: snapshot)
            case .failure(let error): failures[key] = Self.reason(error)
            }
        }
        lastRead = read
        let loaded = keys.sorted().compactMap { read[$0] }
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
            await afterSend()
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
        case EnvelopeError.unknownKind: "This kind of request cannot be answered in this version of the app."
        case let error as DeskReplyError: String(describing: error)
        default: error.localizedDescription
        }
    }
}
