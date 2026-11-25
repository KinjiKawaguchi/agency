#if canImport(SwiftUI) && os(macOS)
import AgencyAPI
import AgencyModel
import Foundation

/// Fixtures for the renderings: the shared sample team and a few requests made to stress the layout.
/// Nothing here is real data.
enum UIFixtures {
    static let now = Date(timeIntervalSince1970: 1_795_521_600 + 3 * 3600)

    static let longJapaneseTitle = "来年度のクラウド基盤をマネージドサービスへ全面的に移行し、既存の自前運用サーバーを段階的に廃止する計画について、予算と体制と移行期間の三点を含めて承認をお願いします"
    static let unbrokenID = "feature-branch-with-a-very-long-name-and-no-spaces-at-all-0123456789-abcdefghijklmnopqrstuvwxyz-0123456789"

    static let document = """
    # 移行計画

    **目的**: 運用の手間を減らし、夜間の呼び出しをなくします。

    - 第 1 段階: ステージングを移す
    - 第 2 段階: 本番を移す (2 週間)
    - 第 3 段階: 旧サーバーを止める

    1. 予算は据え置きです。
    2. 切り戻し手順を用意します。

    > 反対意見があれば、今週中にお知らせください。

    ```
    migrate --dry-run --org acme
    ```

    <script>alert(1)</script> [詳細](https://example.invalid/plan)
    """

    private static func sampleView(from file: String = #filePath) throws -> [String: Any] {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let data = try Data(contentsOf: url.appendingPathComponent("testdata/teamview.sample.json"))
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    static func request(
        id: String, kind: String = "question", title: String, from: String = "lead", to: String? = nil,
        context: String = "", document: String? = nil, blocks: [String] = [], options: [[String: Any]] = [],
        thread: [[String: Any]]? = nil, createdAt: String = "2026-11-24T12:00:00Z"
    ) -> [String: Any] {
        var request: [String: Any] = [
            "id": id, "kind": kind, "title": title, "context": context, "from": from, "createdAt": createdAt,
            "blocks": blocks, "options": options, "answerWith": ["kind": "answer", "target": id],
        ]
        if let to { request["to"] = to }
        if let document { request["document"] = document }
        if let thread { request["thread"] = thread }
        return request
    }

    static func option(_ key: String, _ text: String, recommended: Bool = false, detail: String? = nil) -> [String: Any] {
        var option: [String: Any] = ["key": key, "text": text, "isRecommended": recommended]
        if let detail { option["detail"] = detail }
        return option
    }

    static func team(extra: [[String: Any]] = [], replacing: Bool = false, mine: [String]? = nil) -> Team {
        var view = try! sampleView()
        view["requests"] = replacing ? extra : (view["requests"] as! [[String: Any]]) + extra
        let ids = (view["requests"] as! [[String: Any]]).map { $0["id"] as! String }
        let snapshot: [String: Any] = [
            "view": view, "viewer": "owner", "mine": mine ?? ids, "waitingOnViewer": ["backend"], "source": ["kind": "local"],
        ]
        let data = try! JSONSerialization.data(withJSONObject: snapshot)
        return Team(snapshot: try! JSONDecoder().decode(TeamSnapshot.self, from: data))
    }

    static func item(_ id: String, in team: Team) -> DeskItem {
        Desk.openItems([team]).first { $0.request.id == id }!
    }

    static func decisionLog(_ name: String = "decision-log-member", from file: String = #filePath) -> DecisionLog {
        var url = URL(fileURLWithPath: file)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let data = try! Data(contentsOf: url.appendingPathComponent("testdata/client/\(name).json"))
        return try! JSONDecoder().decode(DecisionLog.self, from: data)
    }
}
#endif
