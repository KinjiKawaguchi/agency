#if canImport(SwiftUI)
import AgencyAPI
import AgencyModel
import SwiftUI

/// Where a request's reply stands.
enum ReplyStatus: Equatable {
    case idle
    case sending
    case sent(SentReply)
    case failed(String)
}

/// One request, open: what it says, its options, and the two things the owner can do with it:
/// decide, or comment without deciding. The content only; `RequestDetailScreen` wires it to the store.
struct RequestDetail: View {
    let item: DeskItem
    let draft: DeskDraft
    let status: ReplyStatus
    let now: Date
    /// Draws the note as plain text, for renderings that cannot draw an editable field.
    var isStatic = false
    let onText: @MainActor @Sendable (String) -> Void
    let onPick: (String) -> Void
    let onDecide: () -> Void
    let onComment: () -> Void

    @State private var showsDocument = false

    private var request: Request { item.request }
    private var hasOptions: Bool { !request.options.isEmpty }
    private var isSent: Bool { if case .sent = status { true } else { false } }
    private var controls: ReplyControls {
        ReplyControls(item: item, draft: draft, isSending: status == .sending, isSent: isSent)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            header
            if !item.blockedPeople.isEmpty {
                Text(Ja.Paper.stoppedBy(item.blockedPeople.map(\.label).joined(separator: "、")))
                    .font(.footnote).foregroundStyle(Theme.warning).wrapping()
            }
            if !request.context.isEmpty {
                Text(request.context).font(.body).wrapping()
            }
            if let document = request.document, !document.isEmpty {
                documentSection(document)
            }
            if let thread = request.thread, !thread.isEmpty {
                threadSection(thread)
            }
            if !item.isMine {
                Text(Ja.Paper.waitingOn(item.decider?.label ?? request.to ?? "—"))
                    .font(.subheadline).foregroundStyle(.secondary).wrapping().card()
            } else {
                answerSection
            }
        }
    }

    // MARK: Reading

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            Tag(text: Ja.Paper.type(request.kind))
            Text(request.title).font(.title2.bold()).multilineTextAlignment(.leading).wrapping()
            Text("\(Ja.Paper.team(org: item.org, team: item.team)) · \(Ja.Desk.drafter(item.drafter?.label ?? request.from)) · \(Ago.text(from: request.createdAt.date, now: now))")
                .font(.footnote).foregroundStyle(.secondary).wrapping()
        }
    }

    private func documentSection(_ document: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                showsDocument.toggle()
            } label: {
                HStack {
                    Text(Ja.Paper.readDocument(characters: document.unicodeScalars.count)).font(.subheadline.weight(.semibold))
                    Spacer(minLength: 0)
                    Image(systemName: showsDocument || isStatic ? "chevron.up" : "chevron.down")
                }
            }
            .buttonStyle(.plain)
            if showsDocument || isStatic { SafeMarkdownView(document: document) }
        }
        .card()
    }

    private func threadSection(_ thread: [ThreadMessage]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(Ja.Paper.earlierMessages).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            ForEach(Array(thread.enumerated()), id: \.offset) { _, message in
                VStack(alignment: .leading, spacing: 2) {
                    Text(Ja.Paper.thread(name: message.from, choice: message.choice)).font(.footnote.weight(.semibold)).wrapping()
                    if !message.text.isEmpty { Text(message.text).font(.footnote).wrapping() }
                }
            }
        }
        .card()
    }

    // MARK: Answering

    private var answerSection: some View {
        VStack(alignment: .leading, spacing: Theme.spacing) {
            if hasOptions {
                Text(Ja.Paper.options).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                ForEach(request.options, id: \.key) { option in
                    OptionCard(option: option, isPicked: draft.optionKey == option.key, isEnabled: controls.canPick) { onPick(option.key) }
                }
            }
            Text(hasOptions ? Ja.Paper.note : Ja.Paper.noteLabelOpen).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            note
            decideButton
            if controls.showsComment { commentSection }
            statusLine
            Label(Ja.Paper.faceIDNote, systemImage: "faceid").font(.caption).foregroundStyle(.secondary).wrapping()
        }
    }

    @ViewBuilder
    private var note: some View {
        let placeholder = hasOptions ? Ja.Paper.notePlaceholderWithOptions : Ja.Paper.notePlaceholderOpen
        if isStatic {
            Text(draft.text.isEmpty ? placeholder : draft.text)
                .font(.body).foregroundStyle(draft.text.isEmpty ? .secondary : .primary).wrapping().card()
        } else {
            TextField(placeholder, text: Binding(get: { draft.text }, set: { onText($0) }), axis: .vertical)
                .lineLimit(3...10)
                .disabled(!controls.canPick)
                .padding(Theme.spacing)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: Theme.corner))
                .overlay(RoundedRectangle(cornerRadius: Theme.corner).stroke(Theme.hairline))
        }
    }

    private var decideButton: some View {
        Button(action: onDecide) {
            Text(controls.decideLabel).font(.headline).multilineTextAlignment(.center).lineLimit(6).wrapping().frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
        .disabled(!controls.canDecide)
    }

    private var commentSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button(action: onComment) {
                Text(Ja.Paper.sendComment).font(.headline).multilineTextAlignment(.center).lineLimit(3).wrapping().frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
            .disabled(!controls.canComment)
            Text(Ja.Paper.commentExplain).font(.footnote).foregroundStyle(.secondary).wrapping()
        }
        .padding(.top, 4)
    }

    @ViewBuilder
    private var statusLine: some View {
        switch status {
        case .idle:
            EmptyView()
        case .sending:
            HStack(spacing: 8) { ProgressView(); Text(Ja.Paper.sending).font(.footnote) }
        case .sent(let reply):
            let drafter = item.drafter?.label ?? request.from
            let text = reply.outcome == .queued
                ? Ja.Paper.queued
                : (reply.isComment ? Ja.Paper.sentComment(to: drafter) : Ja.Paper.sent(to: drafter))
            Label(text, systemImage: "checkmark.circle.fill").font(.footnote).foregroundStyle(.green).wrapping()
        case .failed(let reason):
            Label(Ja.Paper.sendFailed(reason), systemImage: "exclamationmark.triangle.fill")
                .font(.footnote).foregroundStyle(Theme.danger).wrapping()
        }
    }
}
#endif
