import Foundation

/// The words of the widgets, in Japanese, with the glossary's names (依頼, 決めること).
public enum WidgetWording {
    /// The widget's name in the gallery.
    public static let displayName = "決めること"
    public static let description = "あなたの判断を待っている依頼を表示します。"
    public static let nothingWaits = "待っている依頼はありません。"
    public static let notSynced = "アプリを開くと表示されます。"
    public static let waitingLabel = "あなたを待っている依頼"

    public static func count(_ count: Int) -> String { "\(count) 件" }
    public static func inline(_ count: Int) -> String { "決めること \(count) 件" }
    public static func more(_ count: Int) -> String { "ほか \(count) 件" }

    /// How long ago the app wrote the snapshot, once it is old enough to say.
    public static func updated(minutes: Int) -> String {
        minutes < 120 ? "\(minutes) 分前に更新" : minutes < 2 * 24 * 60 ? "\(minutes / 60) 時間前に更新" : "\(minutes / (24 * 60)) 日前に更新"
    }

    public static func kind(_ kind: WidgetRequest.Kind) -> String {
        switch kind {
        case .proposal: "稟議"
        case .question: "質問"
        case .pendingDecision: "判断待ち"
        }
    }
}
