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
    private(set) var nodeSendsPushKinds = true
    /// The overview of every org and team, read from the same client.
    let overview: OverviewStore
    /// The participants and work of every team, read from the same client.
    let people: PeopleStore
    /// The funding sources and each team's token use, read from the same client.
    let costs: CostsStore
    /// What is new since the last read, as banners.
    let news: NewsStore
    /// What the owner wants to be told, for this home node.
    let notifications: NotificationPreferencesStore

    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, signIn: AccessSignInFlow) {
        self.signIn = signIn
        let client = NodeClient(transport: URLSessionTransport(homeNode: homeNode), accessToken: { tokens.current()?.value })
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
        pairing = signer.map { PairingStore(client: client, key: $0.publicKey, label: Ja.Device.label) }
        devices = signer.map { DevicesStore(source: client, adder: DeviceChangeSender(poster: client, signer: $0), ownKey: $0.publicKey) }
        // The screens read the account and every team through one shared cache, so the app asks the node once
        // for what several screens want, and a decision drops what was read before it.
        let sent = SentDecisions()
        let cache = SnapshotCache(source: client)
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
        let pairing = pairing
        let ownKey = signer?.publicKey.string ?? ""
        let news = NewsStore(source: CachedNewsSource(cache: cache, client: client)) {
            NotificationOwn(deviceKey: ownKey, canApprove: pairing?.isReady ?? false, sentDecisionIds: sent.ids)
        }
        notifications = NotificationPreferencesStore(storage: NotificationDefaultsStorage(host: homeNode.host)) { news.enabledKinds = $0.bannerKinds }
        news.enabledKinds = notifications.preferences.bannerKinds
        self.news = news
    }

    /// Runs the Access sign-in again (the session ended, or there is no token yet) and reads the desk.
    func signInAgain() async {
        signInProblem = nil
        switch await signIn.signIn() {
        case .signedIn:
            await desk.refresh()
            await pairing?.refresh()
            await overview.refresh()
            await people.refresh()
            await costs.refresh()
        case .canceled:
            break
        case .unavailable:
            signInProblem = Ja.Desk.signInUnavailable
        }
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
    var errorDescription: String? { Ja.Device.noKey }
}
