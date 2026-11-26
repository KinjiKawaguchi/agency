#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// What waits for the root key's owner to sign, below a started recovery on the desk: how many, what each changes, and the command
/// that signs them together. It is not only a colour: a symbol and the words. It goes when the node stops saying that they wait
/// (signed, or dropped), and nothing is drawn before the node has answered or when none waits.
struct RootChangeBanner: View {
    let notice: RootChangeNotice
    var now: Date = Date()

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: "key.fill").foregroundStyle(Theme.warning).accessibilityHidden(true)
                Text(notice.title).font(.subheadline.weight(.semibold)).wrapping()
            }
            ForEach(Array(notice.lines(now: now).enumerated()), id: \.offset) { _, line in
                Text(line).font(.subheadline).wrapping()
            }
            Text(signTogether).font(.subheadline).textSelection(.enabled).wrapping()
        }
        .card()
        .accessibilityElement(children: .combine)
    }

    /// The sentence, with the command set apart so that it reads as something to type and can be selected whole.
    private var signTogether: AttributedString {
        let parts = notice.signTogether
        var command = AttributedString(parts.command)
        command.font = .system(.footnote, design: .monospaced)
        return AttributedString(parts.before) + command + AttributedString(parts.after)
    }
}
#endif
