import AgencyKit
import AuthenticationServices
import UIKit
import WebKit

/// Signs the owner in to Cloudflare Access in the system's web authentication session, which runs
/// in Safari's context, and gives the resulting Access cookie to the web view.
///
/// A `WKWebView` cannot run WebAuthn for the Access team domain, so passkeys only work there. The
/// session starts on the node's handoff route, and the node returns to a universal link on its own
/// host with the token in the fragment (see `AccessHandoff`).
@MainActor
final class AccessSignInFlow: NSObject {
    enum Outcome {
        case signedIn
        /// The owner closed the sheet.
        case canceled
        /// The session could not run; the web view shows the Access login page instead.
        case unavailable
    }

    /// The host the app was built with in its associated domains (`AGENCY_ASSOCIATED_HOST`).
    static var configuredAssociatedHost: String {
        Bundle.main.object(forInfoDictionaryKey: "AgencyAssociatedHost") as? String ?? ""
    }

    private let homeNode: HomeNodeURL
    private let associatedHost: String
    private let canHandOff: Bool
    private var gate = AccessSignInGate()
    private var session: ASWebAuthenticationSession?

    init(homeNode: HomeNodeURL, associatedHost: String = AccessSignInFlow.configuredAssociatedHost) {
        self.homeNode = homeNode
        self.associatedHost = associatedHost
        canHandOff = AccessHandoff(homeNode: homeNode, associatedHost: associatedHost) != nil
    }

    /// What to do with a login page the web view is about to show.
    func next() -> AccessSignInStep {
        canHandOff ? gate.next(now: Date()) : .loadInWebView
    }

    /// Runs one sign-in. Call it after `next()` answered `.start`.
    func run(cookies: WKHTTPCookieStore) async -> Outcome {
        guard let handoff = AccessHandoff(homeNode: homeNode, associatedHost: associatedHost) else {
            gate.failed()
            return .unavailable
        }
        do {
            let callback = try await authenticate(handoff)
            let token = try handoff.token(from: callback, now: Date())
            guard let httpCookie = Self.httpCookie(for: token) else { throw AccessHandoffError.malformedToken }
            await cookies.setCookie(httpCookie)
            gate.signedIn(at: Date())
            return .signedIn
        } catch let error as ASWebAuthenticationSessionError where error.code == .canceledLogin {
            gate.canceled()
            return .canceled
        } catch {
            gate.failed()
            return .unavailable
        }
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

    private static func httpCookie(for token: AccessToken) -> HTTPCookie? {
        HTTPCookie(properties: [
            .name: AccessToken.cookieName,
            .value: token.value,
            .domain: token.host,
            .path: token.cookiePath,
            .expires: token.expires,
            .secure: "TRUE",
            HTTPCookiePropertyKey("HttpOnly"): "TRUE",
        ])
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
