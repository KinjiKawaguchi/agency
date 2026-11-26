#if canImport(SwiftUI)
import AgencyModel
import SwiftUI

/// A recovery of an org that was started and waits, at the top of the desk. For the owner of the root key it gives the command that
/// cancels it; for any other member it says it and who can cancel it. It has no button to hide it, and it is not only a
/// colour: a symbol and the words. It goes when the node stops saying that the recovery waits (cancelled, or taken).
struct RecoveryBanner: View {
    let notice: RecoveryNotice
    var timeZone: TimeZone = .current

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.warning).accessibilityHidden(true)
                Text(notice.lines(in: timeZone).first ?? "").font(.subheadline.weight(.semibold)).wrapping()
            }
            ForEach(Array(notice.lines(in: timeZone).dropFirst().enumerated()), id: \.offset) { _, line in
                Text(line).font(.subheadline).wrapping()
            }
            if let command = notice.cancelCommand {
                Text(Ja.Recovery.notYours).font(.subheadline).wrapping()
                Text(command)
                    .font(.system(.footnote, design: .monospaced))
                    .textSelection(.enabled)
                    .wrapping()
            }
        }
        .card()
        .overlay(RoundedRectangle(cornerRadius: Theme.corner).stroke(Theme.warning, lineWidth: 2))
        .accessibilityElement(children: .combine)
    }
}
#endif
