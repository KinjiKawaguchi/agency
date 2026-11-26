import AgencyKit
import SwiftUI

/// Asks for the home node address on the first launch.
struct SetupView: View {
    let onSave: (HomeNodeURL) -> Void

    @State private var input = ""
    @State private var errorMessage: String?
    @FocusState private var isFieldFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Spacer()
            Text("Agency")
                .font(.largeTitle.bold())
            Text("Enter the address of your home node. You sign in on the next screen.")
                .foregroundStyle(.secondary)
            TextField("node.example.com", text: $input)
                .textFieldStyle(.roundedBorder)
                #if os(iOS)
                .keyboardType(.URL)
                .textContentType(.URL)
                .textInputAutocapitalization(.never)
                #endif
                .autocorrectionDisabled()
                .submitLabel(.go)
                .focused($isFieldFocused)
                .onSubmit(connect)
            if let errorMessage {
                Text(errorMessage)
                    .font(.footnote)
                    .foregroundStyle(.red)
            }
            Button("Connect", action: connect)
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            Spacer()
            Spacer()
        }
        .padding(24)
        .frame(maxWidth: 480)
        .onAppear { isFieldFocused = true }
    }

    private func connect() {
        do {
            onSave(try HomeNodeURL(validating: input))
        } catch let error as HomeNodeURLError {
            errorMessage = error.message
        } catch {
            errorMessage = HomeNodeURLError.invalid.message
        }
    }
}

extension HomeNodeURLError {
    var message: String {
        switch self {
        case .empty: "Enter the address of your home node."
        case .invalid: "That is not a valid address."
        case .unsupportedScheme: "Use an https address."
        case .hasCredentials: "Leave the user name and password out of the address."
        case .insecureRemoteHost: "Use https. Plain http only works for this device and .local names."
        }
    }
}
