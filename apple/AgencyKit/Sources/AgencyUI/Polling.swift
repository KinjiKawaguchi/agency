#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

extension View {
    /// Reads now and then again every `Ja.Loading.pollSeconds` while the screen is shown: a region that could
    /// not be read is asked again by itself, as on the web client. The task ends when the screen goes away.
    func polling(_ refresh: @escaping @Sendable () async -> Void) -> some View {
        task {
            while !Task.isCancelled {
                await refresh()
                try? await Task.sleep(for: .seconds(Ja.Loading.pollSeconds))
            }
        }
    }
}
#endif
