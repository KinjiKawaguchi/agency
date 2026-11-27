import Foundation

extension Ja {
    /// The node's own response times on the evaluation screen, in the web's words (`metrics` in `ja.ts`) and the glossary's: 応答時間,
    /// ルートの区分, ノード内の読み取り, 目標. A percentile is always 「以下」: it is the upper bound of its bucket.
    public enum NodeSpeed {
        public static let title = "このノードの応答時間"
        public static let lead = "このノードが答えるまでにかかった時間と、このノードが行う呼び出しにかかった時間を、ルートの区分ごとに示します。読めるのはオーナーだけで、数字はこのノードの外には出ません。p50、p95、p99 の時間は、リクエストが入った区間の上限なので「以下」です。それより細かい時間は分かりません。"
        public static let period = "期間"
        public static func days(_ count: Int) -> String { count == 1 ? "直近 1 日" : "直近 \(count) 日" }
        public static let loading = "計測を読み込んでいます…"
        public static let unavailable = "このノードは、まだ応答時間を計測しません。"
        /// The node could not give its measurements, or what it gave could not be read: said plainly, without the reader's own words.
        public static let couldNotAnswer = "ノードが計測を返せませんでした。"
        public static let none = "この期間は、まだ何も計測されていません。"

        /// The last bound of the node's buckets: what is slower than it has no bound.
        static let lastBoundMs = 30_000

        public static func ms(_ count: Int) -> String { "\(JaFormat.integer(count)) ミリ秒" }
        public static func seconds(_ count: String) -> String { "\(count) 秒" }
        public static func atMost(_ value: String) -> String { "\(value) 以下" }
        public static func slowerThan(_ value: String) -> String { "\(value) より遅い" }

        /// `250 ミリ秒`, `2.5 秒`, `10 秒`.
        public static func duration(_ ms: Int) -> String {
            guard ms >= 1000 else { return Self.ms(ms) }
            return seconds(ms % 1000 == 0 ? "\(ms / 1000)" : "\(Double(ms) / 1000)")
        }

        public static func bound(_ value: Bound) -> String {
            switch value {
            case .atMost(let ms): atMost(duration(ms))
            case .beyond: slowerThan(duration(lastBoundMs))
            }
        }

        public enum Budget {
            public static let title = "ノード内の読み取りと予算"
            public static func lead(_ p95: String) -> String { "ノード内の読み取りは、ノードが自分の状態だけから答える読み取りです。予算は、基準の環境で p95 が \(p95) 以下です。" }
            public static let noReads = "この期間は、ノード内の読み取りが計測されていません。"
            public static func within(_ p95: String) -> String { "予算の内です: p95 は \(p95)。" }
            public static func over(_ p95: String) -> String { "予算を超えています: p95 は \(p95)。" }
            public static func undetermined(_ p95: String, _ budget: String) -> String {
                "予算に対して判断できません: p95 は \(p95) で、予算の \(budget) はその範囲の中にあります。"
            }
            public static func requests(_ count: Int) -> String { "\(JaFormat.integer(count)) 件の読み取りを計測しました。" }
            public static let overRoutes = "予算を超えている読み取り"
        }

        public enum Groups {
            public static let title = "このノードが答えるルート（区分ごと）"
            public static let callsTitle = "このノードが行う呼び出し"
            public static let group = "ルートの区分"
            public static let requests = "リクエスト"
            public static let errors = "エラー"
            public static let inNodeRead = "ノード内の読み取り"
            static let names = [
                "desk": "デスク", "teams": "チーム", "usage": "使用量", "evaluation": "評価", "outcomes": "成果の指標", "devices": "端末",
                "push": "プッシュ通知", "federation": "連合", "metrics": "計測", "unmatched": "このノードのルートではないもの", "other": "その他",
                "runtime": "チームのランタイム", "peer": "別のノード", "github": "GitHub",
            ]

            /// The group's own name, or the node's word for a group this app has none for.
            public static func name(_ group: String) -> String { names[group] ?? group }
        }

        /// The hour-by-hour trend, in the web's words (`metrics.trend` in `ja.ts`).
        public enum Trend {
            public static let title = "時間ごとの推移"
            public static let lead = "ルートの区分ごとの、各時間の p95 です。p95 は区間の上限なので「以下」です。目盛りは 1 つで、単位はミリ秒（0.5 ミリ秒から 30 秒）、対数目盛りです。呼び出しがなかった時間は 0 ではなく、線の切れ目になります。"
            public static let approximateNote = "1 点は、いちばん遅いルートの p95 です（このノードは時間ごとの区間の数を返しません）。その区分自身の p95 は、その時間についてこれ以下です。"
            public static let budgetNote = "予算の線より上にある点は、予算を破ったことを言いません。ノード内の読み取りすべてをまとめた p95 は、その点以下です。"
            public static let onlyShort = "ノードが時間ごとの数字を返すのは、7 日までの期間です。もっと短い期間を選ぶと見られます。"
            public static let inNodeReads = "ノード内の読み取り"
            public static func callGroup(_ group: String) -> String { "呼び出し: \(group)" }
            public static func budget(_ value: String) -> String { "予算 \(value)" }
            public static func summary(_ title: String, _ hours: String, _ latest: String) -> String {
                "\(title): ルートをまとめた各時間の p95。呼び出しのあった時間は \(hours) 時間で、直近は \(latest)。"
            }
            public static func summaryApproximate(_ title: String, _ hours: String, _ latest: String) -> String {
                "\(title): 各時間でいちばん遅いルートの p95。呼び出しのあった時間は \(hours) 時間で、直近は \(latest)。"
            }
            public static let show = "時間ごとの数字を見る"
            public static func caption(_ title: String) -> String { "\(title): 呼び出しのあった時間（新しい順）" }
            public static let hour = "時間"
            public static let calls = "呼び出し"
        }

        public enum Objectives {
            public static let title = "目標"
            public static let lead = "運用者がノードの設定で、ルートの区分ごとに決めた応答時間です。この期間に守れたかどうかは、ノードが答えます。"
            public static func target(_ group: String, _ p95: String) -> String { "\(group): p95 \(p95)" }
            public static func measured(_ value: String) -> String { "計測値: \(value)。" }
            public static let met = "達成"
            public static let notMet = "未達"
            public static let undetermined = "この目標に対して判断できません"
            public static let unknown = "まだ判断できません"
        }
    }
}
