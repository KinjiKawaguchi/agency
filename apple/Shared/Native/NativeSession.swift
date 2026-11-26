import AgencyAPI
import AgencyKit
import AgencyModel
import AgencyProtocol
import Foundation
import Observation

/// Everything the native screens need for one home node: the API client with its Keychain token, this
/// phone's Secure Enclave key, the desk and the pairing.
@MainActor
@Observable
final class NativeSession {
    let desk: DeskStore
    /// `nil` when this phone cannot make the key that signs decisions (no Secure Enclave, or Face ID is not set up).
    let pairing: PairingStore?
    let keyProblem: String?
    /// The devices screen's state; `nil` when this phone has no key.
    let devices: DevicesStore?
    private(set) var signInProblem: String?

    /// The client of the home node, for the screens that read from it.
    @ObservationIgnored let client: NodeClient
    @ObservationIgnored private let signIn: AccessSignInFlow
    /// What was last told to the node about the push token, so that a change of the token or of the kinds is told again.
    @ObservationIgnored private var registeredPush: RegisteredPush?
    /// Whether the home node took the kinds of push the owner chose; `false` for a node that sends requests only.
    /// Whether the home node sends the kinds the owner chose, or `nil` until it has answered the registration.
    private(set) var nodeSendsPushKinds: Bool?
    /// The overview of every org and team, read from the same client.
    let overview: OverviewStore
    /// The participants and work of every team, read from the same client.
    let people: PeopleStore
    /// The funding sources and each team's token use, read from the same client.
    let costs: CostsStore
    /// How fast each team ships, and where its effort goes, read from the same client.
    let evaluation: EvaluationStore
    /// The org chart: every org under the account, with the overview's signals on each team.
    let orgChart: OrgChartStore
    /// The owner's numbers for what each org measures: offered only when the node lets the owner read them.
    let outcomes: OutcomesStore
    /// What is new since the last read, as banners.
    let news: NewsStore
    /// What the widgets show, written after the desk is read.
    let widgets: WidgetPublisher
    private let cache: SnapshotCache
    /// What the owner wants to be told, for this home node.
    let notifications: NotificationPreferencesStore

    /// - Parameter deviceLabel: what this device is called in the list of devices (`iPhone`, `Mac`).
    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, signIn: AccessSignInFlow, deviceLabel: String) {
        self.signIn = signIn
        let client = NodeClient(
            transport: URLSessionTransport(homeNode: homeNode), accessToken: { tokens.current()?.value }, clientIdentity: ClientIdentity.current()
        )
        self.client = client
        let signer: SecureEnclaveSigner?
        var problem: String?
        do {
            signer = try SecureEnclaveSigner.loadOrCreate(host: homeNode.host)
        } catch {
            signer = nil
            problem = "\(error)"
        }
        keyProblem = problem
        pairing = signer.map { PairingStore(client: client, key: $0.publicKey, label: deviceLabel) }
        devices = signer.map { DevicesStore(source: client, adder: DeviceChangeSender(poster: client, signer: $0), ownKey: $0.publicKey) }
        // The screens read the account and every team through one shared cache, so the app asks the node once
        // for what several screens want, and a decision drops what was read before it.
        let sent = SentDecisions()
        let cache = SnapshotCache(source: client)
        self.cache = cache
        let cached = CachedSource(cache: cache, client: client)
        desk = DeskStore(
            source: cached,
            sender: signer.map { DeviceDecisionSender(poster: client, signer: $0, sent: sent) as any DeskSender } ?? NoKeySender(),
            storage: DraftDefaultsStorage(host: homeNode.host),
            afterSend: { await cache.invalidate() }
        )
        overview = OverviewStore(source: cached)
        people = PeopleStore(source: cache)
        costs = CostsStore(source: cached)
        evaluation = EvaluationStore(source: CachedEvaluationSource(cache: cache, client: client), nodeSpeed: NodeSpeedStore(source: client))
        orgChart = OrgChartStore(source: CachedOrgChartSource(cache: cache, client: client))
        outcomes = OutcomesStore(source: CachedOutcomesSource(cache: cache, client: client))
        let pairing = pairing
        let ownKey = signer?.publicKey.string ?? ""
        let news = NewsStore(source: CachedNewsSource(cache: cache, client: client)) {
            NotificationOwn(deviceKey: ownKey, canApprove: pairing?.isReady ?? false, sentDecisionIds: sent.ids)
        }
        notifications = NotificationPreferencesStore(storage: NotificationDefaultsStorage(host: homeNode.host)) { news.enabledKinds = $0.bannerKinds }
        news.enabledKinds = notifications.preferences.bannerKinds
        self.news = news
        widgets = WidgetPublisher(
            host: homeNode.host, preferences: WidgetPreferencesStore(storage: WidgetDefaultsStorage(host: homeNode.host)),
            sink: WidgetAppGroupSink()
        )
    }

    /// Runs the Access sign-in again (the session ended, or there is no token yet) and reads the desk.
    func signInAgain() async {
        signInProblem = nil
        switch await signIn.signIn() {
        case .signedIn:
            await desk.refresh()
            await pairing?.refresh()
            // A member the node hosts has the desk and the devices: the owner's screens are not offered, and what they read would be refused.
            guard !desk.standing.isGuest else { break }
            await overview.refresh()
            await people.refresh()
            await costs.refresh()
            await evaluation.refresh()
            await orgChart.refresh()
        case .canceled:
            break
        case .unavailable:
            signInProblem = Ja.Desk.signInUnavailable
        }
    }

    /// The account as the node last said it (through the shared cache), or `nil` when it cannot be read now.
    func readAccount() async -> Me? { try? await cache.me() }

    /// Writes the desk for the widgets, once the node has said whether it keeps titles off devices. When the node cannot be
    /// asked nothing is written, so no title leaves the app on a guess.
    func publishWidgets() async {
        guard let me = try? await cache.me() else { return }
        widgets.preferences.nodeSays(hidesTitles: me.hidesTitles)
        widgets.publish(items: desk.items)
    }

    /// Tells the node this phone's push token and the kinds of notification the owner wants, once the desk can be
    /// read. Registering a known token only updates it, so a change of the kinds is told the same way.
    func registerPush(_ registration: PushRegistration) async {
        let state = RegisteredPush(registration: registration, events: notifications.preferences.pushEventNames)
        guard state != registeredPush, desk.loading == .loaded else { return }
        do {
            let outcome = try await PushRegistrar(poster: client).register(
                token: registration.token, environment: registration.environment.rawValue,
                label: registration.label, events: state.events
            )
            registeredPush = state
            nodeSendsPushKinds = outcome == .withKinds
        } catch {
            // Tried again at the next change or the next launch.
        }
    }
}

private struct RegisteredPush: Equatable {
    let registration: PushRegistration
    let events: [String]
}

/// Stands in for the signer when this phone has no key: the desk can be read and nothing can be sent.
private struct NoKeySender: DeskSender {
    func send(_ decision: PreparedDecision) async throws -> DecisionResult {
        throw NoKeyError()
    }
}

private struct NoKeyError: Error, LocalizedError {
    var errorDescription: String? { Ja.Key.noKey(.platform) }
}
