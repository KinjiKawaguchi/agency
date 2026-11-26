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
    @ObservationIgnored private let transport: URLSessionTransport
    @ObservationIgnored private let signIn: AccessSignInFlow
    @ObservationIgnored private var registeredPush: PushRegistration?
    /// The overview of every org and team, read from the same client.
    let overview: OverviewStore
    /// The participants and work of every team, read from the same client.
    let people: PeopleStore
    /// The funding sources and each team's token use, read from the same client.
    let costs: CostsStore

    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, signIn: AccessSignInFlow) {
        self.signIn = signIn
        let transport = URLSessionTransport(homeNode: homeNode)
        self.transport = transport
        let client = NodeClient(transport: transport, accessToken: { tokens.current()?.value })
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
        desk = DeskStore(
            source: client,
            sender: signer.map { DeviceDecisionSender(poster: client, signer: $0) as any DeskSender } ?? NoKeySender(),
            storage: DraftDefaultsStorage(host: homeNode.host)
        )
        overview = OverviewStore(source: client)
        people = PeopleStore(source: client)
        costs = CostsStore(source: client)
    }

    /// The node's Access team host, from where the node sends a browser that is not signed in. It is the
    /// only Access host the web view may show.
    func discoverAccessTeam() async -> String? {
        await transport.accessRedirectLocation().flatMap(AccessTeam.host(fromRedirect:))
    }

    /// Runs the Access sign-in again (the session ended, or there is no token yet) and reads the desk.
    func signInAgain() async {
        signInProblem = nil
        switch await signIn.signInAgain() {
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

    /// Tells the node this phone's push token, once the desk can be read. Registering a known token only updates it.
    func registerPush(_ registration: PushRegistration) async {
        guard registration != registeredPush, desk.loading == .loaded else { return }
        do {
            try await client.registerPushToken(token: registration.token, environment: registration.environment.rawValue, label: registration.label)
            registeredPush = registration
        } catch {
            // Tried again at the next change or the next launch.
        }
    }
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
