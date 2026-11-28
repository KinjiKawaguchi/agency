import AgencyAPI
import Foundation

extension Ja {
    /// Words for work that can or cannot move, shared by the screens that list it.
    public enum Work {
        public static func blockedReason(_ reason: BlockedReason) -> String {
            switch reason {
            case .needsDecision: "相談待ち"
            case .external: "外部の対応待ち"
            case .waitingOnPrevious: "先行する仕事待ち"
            case .timeOrEquipment: "時間か機材待ち"
            case .unknown: "待機中"
            }
        }
    }
}
