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
    private let tokens: AccessTokenStore
    private let canHandOff: Bool
    private var gate = AccessSignInGate()
    private var session: ASWebAuthenticationSession?

    init(homeNode: HomeNodeURL, tokens: AccessTokenStore, associatedHost: String = AccessSignInFlow.configuredAssociatedHost) {
        self.homeNode = homeNode
        self.tokens = tokens
        self.associatedHost = associatedHost
        canHandOff = AccessHandoff(homeNode: homeNode, associatedHost: associatedHost) != nil
    }

    /// What to do with a login page the web view is about to show.
    func next() -> AccessSignInStep {
        canHandOff ? gate.next(now: Date()) : .loadInWebView
    }

    /// Runs one sign-in. Call it after `next()` answered `.start`.
    func run() async -> Outcome {
        await perform()
    }

    /// Runs a sign-in the owner asked for (from the desk), which the loop guard does not hold back.
    func signInAgain() async -> Outcome {
        guard canHandOff else { return .unavailable }
        return await perform()
    }

    /// Forgets the sign-in of the home node: the token and the cookie. The session in Safari is left alone.
    func forget() async {
        tokens.clear()
        let cookies = WKWebsiteDataStore.default().httpCookieStore
        for cookie in await cookies.allCookies() where cookie.name == AccessToken.cookieName {
            let host = cookie.domain.hasPrefix(".") ? String(cookie.domain.dropFirst()) : cookie.domain
            if host.lowercased() == homeNode.host { await cookies.deleteCookie(cookie) }
        }
    }

    /// The token goes to the Keychain, for the API calls, and to the web view as the cookie Access itself
    /// would set, for the screens that are still web pages.
    private func perform() async -> Outcome {
        let cookies = WKWebsiteDataStore.default().httpCookieStore
        guard let handoff = AccessHandoff(homeNode: homeNode, associatedHost: associatedHost) else {
            gate.failed()
            return .unavailable
        }
        do {
            let callback = try await authenticate(handoff)
            let token = try handoff.token(from: callback, now: Date())
            guard let httpCookie = Self.httpCookie(for: token) else { throw AccessHandoffError.malformedToken }
            tokens.save(token)
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
