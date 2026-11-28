import AgencyAPI
import Foundation

extension Ja {
    /// A funding source and its usage windows, from the web client's Japanese (`costs` in `ja/costs.ts`).
    public enum Funding {
        public static let unknownUsage = "この資金源の使用状況は読み取れませんでした。"

        public static func kind(_ kind: AgencyAPI.Funding.Kind) -> String {
            switch kind {
            case .subscription: "サブスクリプション"
            case .apiKey: "API キー"
            case .unknown: "ほかの資金源"
            }
        }

        public static func window(_ name: UsageWindow.Name) -> String {
            switch name {
            case .sevenDay: "週の枠"
            case .fiveHour: "5 時間の枠"
            case .unknown: "ほかの枠"
            }
        }

        public static func status(_ status: WindowStatus) -> String {
            switch status {
            case .waste: "余りそう"
            case .ok: "順調"
            case .overrun: "足りなくなりそう"
            }
        }

        public static func verdict(_ status: WindowStatus) -> String {
            switch status {
            case .waste: "このペースでは、リセットまでに使い切れません。重い仕事をここに寄せられます。"
            case .ok: "このペースなら、リセットのころに使い切ります。"
            case .overrun: "リセットの前に上限に届き、届くとこの資金源の AI エージェントはすべて止まります。"
            }
        }

        public static func notStarted(_ window: String) -> String { "\(window)はまだ始まっていません。" }
        public static func stale(_ window: String) -> String { "\(window): 最後の読み取りがリセットより前のため、見込みはまだありません。" }
        public static func left(_ percent: String) -> String { "残り \(percent)" }
        public static func readAt(_ when: String) -> String { "\(when) に読み取り" }
        public static func hitLine(_ when: String) -> String { "\(when) ごろに上限に届きます。" }

        public static func barLabel(_ window: String, used: String, projected: String) -> String {
            "\(window): \(used) 使用、リセット時に約 \(projected)"
        }

        public static func resetLine(_ when: String, relative: String, projected: String) -> String {
            "リセットは \(when) (\(relative))。このペースなら、そのとき約 \(projected) です。"
        }
    }
}
