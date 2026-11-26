#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// A region that keeps failing: what could not be read and why, when it is asked again, and a button. Only
/// this region says so; the rest of the screen stays as it is.
struct FailedRegion: View {
    let what: String
    let why: String
    /// Asks now. `nil` draws no button, for renderings.
    var onTryAgain: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Ja.Loading.couldNotLoad(what, why: why)).font(.subheadline).foregroundStyle(Theme.danger).wrapping()
            Text(Ja.Loading.nextTry()).font(.caption).foregroundStyle(.secondary).wrapping()
            if let onTryAgain { Button(Ja.Loading.tryAgain, action: onTryAgain).buttonStyle(.bordered).font(.subheadline) }
        }
    }
}
#endif
