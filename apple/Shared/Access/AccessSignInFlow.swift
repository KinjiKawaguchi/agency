import AgencyAPI
import AgencyKit
import AgencyModel
import AuthenticationServices
import Foundation

/// Signs the owner in to Cloudflare Access for any home node, in the system's web authentication session, which runs in
/// Safari's context, where passkeys and password managers work, and keeps the resulting Access token in the Keychain.
///
/// The session starts on the node's handoff route with a challenge, and the node returns a single-use code (never the
/// token) to a universal link on its own host when the build has an associated domain for it, or to the app's custom
/// scheme for any other host. The app redeems the code at that node with a verifier that never left it, and only then has
/// the token (`SignInExchange`).
@MainActor
final class AccessSignInFlow: NSObject {
    enum Outcome {
        case signedIn
        /// The owner closed the sheet.
        case canceled
        /// The session could not run (the node is not on https), or the node does not take part in the exchange.
        case unavailable
    }

    /// What the build has in its associated domains (`AGENCY_ASSOCIATED_HOST` and the second one, each a host or a
    /// `*.` wildcard), by the Info keys that `project.yml` fills.
    static var configuredAssociatedHosts: [String] {
        guard let listed = Bundle.main.object(forInfoDictionaryKey: "AgencyAssociatedHosts") as? String else { return [] }
        return listed.split(whereSeparator: \.isWhitespace).map(String.init).filter { $0 != "agency.invalid" }
    }

    private let homeNode: HomeNodeURL
    private let associatedHosts: [String]
    private let tokens: AccessTokenStore
    private var session: ASWebAuthenticationSession?

    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, associatedHosts: [String] = AccessSignInFlow.configuredAssociatedHosts) {
        self.homeNode = homeNode
        self.tokens = tokens
        self.associatedHosts = associatedHosts
    }

    /// Runs one sign-in: the owner signs in to Access in the system session, the code it returns is redeemed at this node,
    /// and the token goes to the Keychain under this node's host.
    func signIn() async -> Outcome {
        guard let exchange = SignInExchange(homeNode: homeNode, associatedHosts: associatedHosts), exchange.host == homeNode.host else {
            return .unavailable
        }
        do {
            let callback = try await authenticate(exchange)
            let code = try exchange.code(from: callback)
            // Redeemed at the node it was made for, and nowhere else: the client is this node's.
            let client = NodeClient(transport: URLSessionTransport(homeNode: homeNode), accessToken: { nil }, clientIdentity: ClientIdentity.current())
            let jwt = try await exchange.redeem(code: code, using: client)
            tokens.save(try AccessHandoff.token(fromJWT: jwt, host: exchange.host, now: Date()))
            return .signedIn
        } catch let error as ASWebAuthenticationSessionError where error.code == .canceledLogin {
            return .canceled
        } catch {
            return .unavailable
        }
    }

    /// Forgets the sign-in of the home node: the token. The session in Safari is left alone.
    func forget() {
        tokens.clear()
    }

    private func authenticate(_ exchange: SignInExchange) async throws -> URL {
        let callback: ASWebAuthenticationSession.Callback
        if let scheme = exchange.callbackScheme {
            callback = .customScheme(scheme)
        } else if let host = exchange.callbackHost {
            callback = .https(host: host, path: exchange.callbackPath)
        } else {
            throw SignInExchangeError.notTheCallback
        }
        return try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(url: exchange.startURL, callback: callback) { url, error in
                if let url {
                    continuation.resume(returning: url)
                } else {
                    continuation.resume(throwing: error ?? SignInExchangeError.notTheCallback)
                }
            }
            session.presentationContextProvider = self
            self.session = session
            if !session.start() {
                self.session = nil
                continuation.resume(throwing: SignInExchangeError.notTheCallback)
            }
        }
    }
}
