import AgencyKit
import AuthenticationServices
import UIKit

/// Signs the owner in to Cloudflare Access in the system's web authentication session, which runs
/// in Safari's context, where passkeys and password managers work, and keeps the resulting Access
/// token in the Keychain.
///
/// The session starts on the node's handoff route, and the node returns to a universal link on its own
/// host with the token in the fragment (see `AccessHandoff`).
@MainActor
final class AccessSignInFlow: NSObject {
    enum Outcome {
        case signedIn
        /// The owner closed the sheet.
        case canceled
        /// The session could not run (the node's host is not the one this build is associated with, or the
        /// node has no handoff route).
        case unavailable
    }

    /// The host the app was built with in its associated domains (`AGENCY_ASSOCIATED_HOST`).
    static var configuredAssociatedHost: String {
        Bundle.main.object(forInfoDictionaryKey: "AgencyAssociatedHost") as? String ?? ""
    }

    private let homeNode: HomeNodeURL
    private let associatedHost: String
    private let tokens: AccessTokenStore
    private let canHandOff: Bool
    private var session: ASWebAuthenticationSession?

    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, associatedHost: String = AccessSignInFlow.configuredAssociatedHost) {
        self.homeNode = homeNode
        self.tokens = tokens
        self.associatedHost = associatedHost
        canHandOff = AccessHandoff(homeNode: homeNode, associatedHost: associatedHost) != nil
    }

    /// Runs one sign-in: the owner signs in to Access in the system session and the token goes to the Keychain.
    func signIn() async -> Outcome {
        guard let handoff = AccessHandoff(homeNode: homeNode, associatedHost: associatedHost) else { return .unavailable }
        do {
            let callback = try await authenticate(handoff)
            tokens.save(try handoff.token(from: callback, now: Date()))
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

    private func authenticate(_ handoff: AccessHandoff) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: handoff.startURL,
                callback: .https(host: handoff.callbackHost, path: handoff.callbackPath)
            ) { url, error in
                if let url {
                    continuation.resume(returning: url)
                } else {
                    continuation.resume(throwing: error ?? AccessHandoffError.notTheCallback)
                }
            }
            session.presentationContextProvider = self
            self.session = session
            if !session.start() {
                self.session = nil
                continuation.resume(throwing: AccessHandoffError.notTheCallback)
            }
        }
    }
}

extension AccessSignInFlow: ASWebAuthenticationPresentationContextProviding {
    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .flatMap(\.windows)
                .first(where: \.isKeyWindow) ?? ASPresentationAnchor()
        }
    }
}
