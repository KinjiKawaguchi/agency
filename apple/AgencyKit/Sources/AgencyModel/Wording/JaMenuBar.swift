import Foundation

/// What the Mac's menu-bar item can say, and the only places a number or a plain "not read" may come from: with no
/// home node, before the node has answered, after a read that failed, or with the last number the node gave (which a
/// failed read leaves standing, marked as such).
public enum MenuBarState: Equatable, Sendable {
    /// No home node was entered: there is nothing to read.
    case noHomeNode
    /// A home node, and the node has not answered yet.
    case notRead
    /// A home node, and the first read failed: there is no number.
    case readFailed
    /// The last number the node gave; `isStale` when the read after it failed.
    case count(Int, isStale: Bool)

    /// - Parameters:
    ///   - hasHomeNode: a home node was entered.
    ///   - known: how many requests wait, once the node has answered (`DeskStore.knownWaitingCount`).
    ///   - isFailed: the last read of the desk failed.
    public init(hasHomeNode: Bool, known: Int?, isFailed: Bool) {
        guard hasHomeNode else { self = .noHomeNode; return }
        guard let known else { self = isFailed ? .readFailed : .notRead; return }
        self = .count(known, isStale: isFailed)
    }
}

extension Ja {
    /// The Mac's menu-bar item.
    public enum MenuBar {
        public static let open = "Agency を開く"
        public static let quit = "Agency を終了"
        public static let noHomeNode = "ホームノードが設定されていません"
        public static let notRead = "まだ読み込んでいません"
        public static let readFailed = "読み込めませんでした"
        public static let staleNote = "最新の数を読み込めませんでした。前に読み込んだときの数です"
        public static let nothingWaiting = "あなたを待っているリクエストはありません"
        public static func waiting(_ count: Int) -> String { "\(count) 件があなたを待っています" }

        /// The text of the item's label beside its symbol: the number when something waits, nothing otherwise.
        public static func label(_ state: MenuBarState) -> String? {
            guard case .count(let count, _) = state, count > 0 else { return nil }
            return "\(count)"
        }

        /// Whether the item is drawn dim: the number is not the latest.
        public static func isDim(_ state: MenuBarState) -> Bool {
            switch state {
            case .count(_, let isStale): isStale
            case .readFailed: true
            case .noHomeNode, .notRead: false
            }
        }

        /// The menu's first line.
        public static func status(_ state: MenuBarState) -> String {
            switch state {
            case .noHomeNode: noHomeNode
            case .notRead: notRead
            case .readFailed: readFailed
            case .count(let count, _): count == 0 ? nothingWaiting : waiting(count)
            }
        }

        /// The menu's second line when the number is not the latest, and nothing otherwise.
        public static func note(_ state: MenuBarState) -> String? {
            if case .count(_, true) = state { staleNote } else { nil }
        }

        /// The symbol of the item: a warning while a recovery of an org waits (the owner can cancel it only until it takes effect, and
        /// the menu says which), the full tray when requests wait, the empty tray otherwise.
        public static func symbol(_ state: MenuBarState, hasRecovery: Bool) -> String {
            if hasRecovery { return "exclamationmark.triangle.fill" }
            return label(state) == nil ? "tray" : "tray.full"
        }

        /// What a screen reader says of the item: the state, and a line for each recovery that was started
        /// (`RecoveryNotice.headline`).
        public static func accessibility(_ state: MenuBarState, recoveries: [String] = []) -> String {
            (["Agency: \(status(state))", note(state)].compactMap { $0 } + recoveries).joined(separator: "。")
        }
    }
}
