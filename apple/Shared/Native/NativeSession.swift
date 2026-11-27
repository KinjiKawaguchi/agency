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
    /// This device's key is stored and cannot be used now: the whole app says so, and nothing is decided or signed meanwhile.
    let keyUnusable: KeyProblem?
    /// The devices screen's state; `nil` when this phone has no key.
    let devices: DevicesStore?
    /// Whether APNs takes the notices the node sends this phone, read from the node's list of the owner's tokens.
    let pushDelivery: PushDeliveryStore
    private(set) var signInProblem: String?
    /// Whether this app and the home node still fit each other, once the node has said what it serves.
    private(set) var compatibility: NodeCompatibility = .compatible

    /// The client of the home node, for the screens that read from it.
    @ObservationIgnored let client: NodeClient
    @ObservationIgnored private let signIn: AccessSignInFlow
    /// What was last told to the node about the push token, so that a change of the token or of the kinds is told again.
    @ObservationIgnored private var registeredPush: RegisteredPush?
    /// This phone's push token, once Apple has given it one: what `pushDelivery` looks for among the node's listed tokens.
    @ObservationIgnored private let ownPushToken = PushTokenBox()
    /// What the home node said of itself, once it has: what it serves, and whether it takes `device` in a registration.
    @ObservationIgnored private var nodeAbout: About?
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
    /// - Parameters:
    ///   - key: this device's key for the home node, as the loader found it: in use, stored and unusable now (nothing is replaced and the
    ///     person is asked), or not makeable here (the desk can only be read).
    ///   - standing: what the node said the viewer is, for a session that replaces one of the same node and account (a key reset); the
    ///     screens the viewer had are then not closed while the node is asked again.
    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, signIn: AccessSignInFlow, deviceLabel: String, key: KeyLoad, standing: Standing = .loading) {
        self.signIn = signIn
        let client = NodeClient(
            transport: URLSessionTransport(homeNode: homeNode), accessToken: { tokens.current()?.value }, clientIdentity: ClientIdentity.current()
        )
        self.client = client
        let signer: (any DeviceSigner)?
        var problem: String?
        var unusable: KeyProblem?
        switch key {
        case .ready(let ready): signer = ready
        case .cannotMake(let reason): (signer, problem) = (nil, reason)
        case .unusable(let reason): (signer, unusable) = (nil, reason)
        }
        keyProblem = problem
        keyUnusable = unusable
        pairing = signer.map { PairingStore(client: client, key: $0.publicKey, label: deviceLabel) }
        devices = signer.map { signer in
            let sender = DeviceChangeSender(poster: client, signer: signer)
            return DevicesStore(source: client, adder: sender, ownKey: signer.publicKey, remover: sender)
        }
        let ownToken = ownPushToken
        pushDelivery = PushDeliveryStore(source: client, ownToken: { ownToken.value })
        // The screens read the account and every team through one shared cache, so the app asks the node once
        // for what several screens want, and a decision drops what was read before it.
        let sent = SentDecisions()
        let cache = SnapshotCache(source: client)
        self.cache = cache
        let cached = CachedSource(cache: cache, client: client)
        desk = DeskStore(
            source: cached,
            signer: signer.map { DeviceDecisionSigner(poster: client, signer: $0, sent: sent) as any DeskSigner } ?? NoKeySigner(),
            outbox: DecisionOutbox(storage: FileOutboxStorage(host: homeNode.host), submitter: client),
            storage: DraftDefaultsStorage(host: homeNode.host),
            afterSend: { await cache.invalidate() },
            standing: standing
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
            // What was decided while the sign-in was over goes now.
            await desk.outbox.signInRan()
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

    /// Compares what the home node serves with this app's Client API version and own version, and keeps the answer for the desk to say. A node
    /// that cannot be read now is judged at the next desk read.
    func checkNodeVersion() async {
        await readAbout()
        compatibility = NodeCompatibility.judge(
            about: nodeAbout, clientApi: ClientApi.spoken, appVersion: ClientIdentity.currentVersion(), platform: ClientIdentity.platform
        )
    }

    /// What the node says of itself, read until it has answered: a node that has no such route (404) has no capabilities, once and for good,
    /// and any other failure is tried again at the next registration.
    private func readAbout() async {
        guard nodeAbout == nil else { return }
        do {
            nodeAbout = try await client.about()
        } catch NodeError.http(let status, _) where status == 404 {
            nodeAbout = About(capabilities: [])
        } catch {
            // Not registered until it has been read.
        }
    }

    /// Tells the node this phone's push token and the kinds of notification the owner wants, once the desk can be
    /// read. Registering a known token only updates it, so a change of the kinds is told the same way.
    func registerPush(_ registration: PushRegistration) async {
        ownPushToken.value = registration.token
        guard desk.loading == .loaded else { return }
        await readAbout()
        let device: String?
        switch PushDeviceChoice.decide(about: nodeAbout, thisDeviceID: devices?.thisDeviceID, registered: registeredPush?.device) {
        case .notYet: return
        case .withoutDevice: device = nil
        case .withDevice(let id): device = id
        }
        let state = RegisteredPush(registration: registration, events: notifications.preferences.pushEventNames, device: device)
        guard state != registeredPush else { return }
        do {
            let outcome = try await PushRegistrar(poster: client).register(
                token: registration.token, environment: registration.environment.rawValue,
                label: registration.label, events: state.events, device: device
            )
            registeredPush = state
            nodeSendsPushKinds = outcome == .withKinds
        } catch NodeError.http(let status, _) where status == 400 && device != nil {
            // The node refused the device: it is not one of the account's now, and the list is read again. Not registered without it. A 400
            // for another reason reads the list again at each try, which is a read and nothing more.
            await devices?.refresh()
        } catch {
            // Tried again at the next change or the next launch.
        }
    }
}

/// A place for this phone's push token, which `NativeSession` learns after it has built what asks for it.
@MainActor
private final class PushTokenBox {
    var value: String?
}

private struct RegisteredPush: Equatable {
    let registration: PushRegistration
    let events: [String]
    /// The device the registration names, for a node that takes one.
    let device: String?
}

/// Stands in for the signer when this phone has no key: the desk can be read and nothing can be decided.
private struct NoKeySigner: DeskSigner {
    func sign(_ decision: PreparedDecision) async throws -> SignedDecision {
        throw NoKeyError()
    }
}

private struct NoKeyError: Error, LocalizedError {
    var errorDescription: String? { Ja.Key.noKey(.platform) }
}
