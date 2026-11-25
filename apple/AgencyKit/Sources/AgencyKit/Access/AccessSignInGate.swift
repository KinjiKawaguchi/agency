import Foundation

/// What to do when the web view is about to show an Access login page.
public enum AccessSignInStep: Equatable, Sendable {
    /// Start the sign-in session and keep the login page out of the web view.
    case start
    /// A session is already open: keep the login page out of the web view.
    case wait
    /// Let the web view load the login page, which is how the one-time PIN works.
    case loadInWebView
}

/// Decides when the app signs in through the system session and when it leaves the login page to
/// the web view.
///
/// The session is left alone after a failure that is not the owner canceling (the node may not
/// have the handoff route), and for a short time after a sign-in that worked: if Access asks again
/// right away, the token did not take (for example when the application binds the cookie to the
/// browser), and starting again would loop.
public struct AccessSignInGate: Sendable {
    public static let repeatWindow: TimeInterval = 120

    private var isRunning = false
    private var isUnavailable = false
    private var lastSignIn: Date?

    public init() {}

    public mutating func next(now: Date) -> AccessSignInStep {
        if isRunning { return .wait }
        if isUnavailable { return .loadInWebView }
        if let lastSignIn, now.timeIntervalSince(lastSignIn) < Self.repeatWindow { return .loadInWebView }
        isRunning = true
        return .start
    }

    public mutating func signedIn(at now: Date) {
        isRunning = false
        lastSignIn = now
    }

    /// The owner closed the sign-in sheet.
    public mutating func canceled() {
        isRunning = false
    }

    /// The session could not run or what it returned was unusable.
    public mutating func failed() {
        isRunning = false
        isUnavailable = true
    }
}
