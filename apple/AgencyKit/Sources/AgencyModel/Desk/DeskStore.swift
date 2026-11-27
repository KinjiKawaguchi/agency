import AgencyAPI
import AgencyProtocol
import Foundation
import Observation

/// Where the desk reads from: the home node's Client API. `NodeClient` is the real one.
public protocol DeskSource: TeamSnapshotSource {
    func decisions(limit: Int) async throws -> DecisionLog
}

extension NodeClient: DeskSource {}

/// Signs a prepared decision with this device's key. The app's implementation asks for Face ID; tests use a stand-in. Sending is the
/// outbox's: a signed decision is kept and sent from there, as often as it takes.
public protocol DeskSigner: Sendable {
    func sign(_ decision: PreparedDecision) async throws -> SignedDecision
}

/// Where unsent drafts are kept between launches.
public protocol DraftStorage: Sendable {
    func load() -> [String: DeskDraft]
    func save(_ drafts: [String: DeskDraft])
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

    /// What the node's views hold, before the decisions the owner has made and the node has not yet taken are taken off.
    private var openItems: [DeskItem] = []
    /// The pile: what waits for the owner. A decision that is kept and not yet taken is not in it, and does not come back while its
    /// entry exists (pending, sending or refused), though the node's own view still shows the request.
    public var items: [DeskItem] {
        let decided = decidedItemIDs
        return openItems.filter { !decided.contains($0.id) }
    }

    /// The items the owner has decided that are not settled yet: kept on this device and not yet taken, or taken by the node and still `queued` in
    /// its decision log (the node's own view lists the request until the team has applied it, and it must not come back meanwhile).
    public var decidedItemIDs: Set<String> {
        var ids = outbox.decidedItemIDs
        for entry in recent {
            if case .queued = entry.state { ids.insert(DeskItem.key(org: entry.org, team: entry.team, request: entry.target)) }
        }
        return ids
    }
    public private(set) var teams: [Team] = []
    public private(set) var recent: [DecisionEntry] = []
    /// The recoveries the node says wait, one for each org: shown above the desk until the node stops saying it. What was
    /// said last stays when a later read fails.
    public private(set) var recoveries: [RecoveryNotice] = []
    /// What waits for the root key's owner to sign, once the node has said; `nil` when nothing waits or before it answered.
    public private(set) var rootChanges: RootChangeNotice?
    /// What the node says the viewer is (a member it hosts, or anyone else), once it has said: it decides which screens are offered.
    public private(set) var standing: Standing = .loading
    public private(set) var recentFailure: String?
    public private(set) var loading: Loading = .idle
    /// Teams whose view could not be read, by `org/team`, with the reason.
    public private(set) var teamFailures: [String: String] = [:]
    /// The Access session ended: the sign-in has to run again before anything else works.
    public private(set) var needsSignIn = false
    public private(set) var drafts: [String: DeskDraft]
    /// The items being signed now (the owner is confirming with Face ID).
    public private(set) var sending: Set<String> = []
    /// Why the last attempt to decide an item failed (it could not be signed or kept). The draft is kept.
    public private(set) var sendFailures: [String: String] = [:]

    @ObservationIgnored private let source: any DeskSource
    @ObservationIgnored private let signer: any DeskSigner
    /// The decisions kept on this device until the node has taken them.
    public let outbox: DecisionOutbox
    @ObservationIgnored private let storage: any DraftStorage
    @ObservationIgnored private let afterSend: @Sendable () async -> Void
    @ObservationIgnored private let retry: RetryPolicy
    /// The last view read of each team, kept when a later read of it fails.
    @ObservationIgnored private var lastRead: [String: Team] = [:]

    /// - Parameter afterSend: runs after a decision was sent and before the desk is read again, to drop
    ///   what a shared cache holds from before it.
    /// - Parameter retry: how a team whose view is not ready is asked again, while the others are shown.
    public init(
        source: any DeskSource, signer: any DeskSigner, outbox: DecisionOutbox, storage: any DraftStorage,
        afterSend: @escaping @Sendable () async -> Void = {}, retry: RetryPolicy = .standard
    ) {
        self.source = source
        self.signer = signer
        self.outbox = outbox
        self.storage = storage
        self.afterSend = afterSend
        self.retry = retry
        drafts = storage.load()
        // A decision the node took is read again, so that the pile and the counts follow the node's own view.
        outbox.didTake = { [weak self] in
            await afterSend()
            await self?.refresh()
        }
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
    public var waitingCount: Int { Desk.waitingCount(teams, excluding: decidedItemIDs) }

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
        openItems = Desk.openItems(loaded)
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

    /// Decides: signs the reply (Face ID), keeps the signed envelope on the device, and is done. The item leaves the pile at once; the node
    /// is told from the outbox, as often as it takes, and the owner does not wait for it. The draft is kept when the reply cannot be
    /// signed or kept. Nothing is decided twice for one item.
    public func send(_ reply: DeskReply, for id: String) async {
        guard let item = item(id), !sending.contains(id) else { return }
        let decision: PreparedDecision
        do { decision = try item.prepare(reply) } catch {
            sendFailures[id] = Self.reason(error)
            return
        }
        sending.insert(id)
        sendFailures[id] = nil
        defer { sending.remove(id) }
        do {
            let signed = try await signer.sign(decision)
            try outbox.keep(PendingDecision(itemID: id, signed: signed, isComment: decision.isComment, createdAt: Date()))
            drafts[id] = nil
            storage.save(drafts)
            // Sent from the outbox, which the owner is not waiting for.
            Task { await outbox.flush() }
        } catch NodeError.sessionExpired {
            needsSignIn = true
            sendFailures[id] = "The sign-in has ended."
        } catch OutboxError.notKept {
            sendFailures[id] = "The decision could not be kept on this device."
        } catch {
            sendFailures[id] = Self.reason(error)
        }
    }

    /// The owner lets go of a decision the node refused for good: the item returns to the pile with what they typed and picked.
    public func discardDecision(_ id: String) {
        guard let entry = outbox.entries.first(where: { $0.id == id }), outbox.discard(id) else { return }
        drafts[entry.itemID] = DeskDraft(text: entry.text, optionKey: entry.choice)
        storage.save(drafts)
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
