#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// One option of a request: the owner taps it to pick it, and taps it again to let go.
struct OptionCard: View {
    let option: RequestOption
    let isPicked: Bool
    let isEnabled: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: isPicked ? "checkmark.circle.fill" : "circle")
                    .font(.title3)
                    .foregroundStyle(isPicked ? Color.accentColor : .secondary)
                VStack(alignment: .leading, spacing: 4) {
                    Text(option.text)
                        .font(.body.weight(.semibold))
                        .multilineTextAlignment(.leading)
                        .wrapping()
                    if option.isRecommended {
                        Tag(text: Ja.Paper.recommended, color: .green)
                    }
                    if let detail = option.detail {
                        Text(detail)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.leading)
                            .wrapping()
                    }
                }
            }
            .card(selected: isPicked)
        }
        .buttonStyle(.plain)
        .disabled(!isEnabled)
        .accessibilityAddTraits(isPicked ? .isSelected : [])
    }
}
#endif
