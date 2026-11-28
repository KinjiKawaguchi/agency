import Foundation

/// The routes a client may name when it reports what it observed (`POST /api/telemetry`), as `testdata/route-templates.json` lists them: a
/// route is a method and a path whose ids are `{name}` (`GET /api/orgs/{org}/teams/{team}/view`). A call to anything else is not reported.
public enum RouteTemplates {
    public static let all: [String] = [
        "GET /api/about",
        "GET /api/me",
        "GET /api/session",
        "GET /api/decisions",
        "GET /api/devices",
        "POST /api/devices/requests",
        "POST /api/devices/changes",
        "GET /api/metrics",
        "GET /api/orgs/{org}",
        "GET /api/orgs/{org}/effort",
        "GET /api/orgs/{org}/evaluation",
        "GET /api/orgs/{org}/outcomes",
        "GET /api/orgs/{org}/outcomes/observations",
        "POST /api/orgs/{org}/outcomes/observations",
        "GET /api/orgs/{org}/outcomes/readings",
        "GET /api/orgs/{org}/teams/{team}/activity",
        "GET /api/orgs/{org}/teams/{team}/budget",
        "POST /api/orgs/{org}/teams/{team}/envelopes",
        "GET /api/orgs/{org}/teams/{team}/usage",
        "GET /api/orgs/{org}/teams/{team}/view",
        "GET /api/push/subscriptions",
        "POST /api/push/subscriptions",
        "DELETE /api/push/subscriptions/{id}",
        "GET /api/push/tokens",
        "POST /api/push/tokens",
        "DELETE /api/push/tokens/{token}",
        "GET /api/push/vapid",
    ]

    private static let segmentsOf: [(template: String, method: Substring, segments: [Substring])] = all.compactMap { template in
        let parts = template.split(separator: " ", maxSplits: 1)
        guard parts.count == 2 else { return nil }
        return (template, parts[0], parts[1].split(separator: "/", omittingEmptySubsequences: false))
    }

    /// The template a call to `path` (with its query, if any) by `method` is an instance of, or `nil` when it is none of them.
    public static func template(method: String, path: String) -> String? {
        let withoutQuery = path.prefix { $0 != "?" && $0 != "#" }
        let segments = withoutQuery.split(separator: "/", omittingEmptySubsequences: false)
        return segmentsOf.first { entry in
            entry.method == method && entry.segments.count == segments.count
                && zip(entry.segments, segments).allSatisfy { pattern, segment in
                    pattern.hasPrefix("{") ? !segment.isEmpty : pattern == segment
                }
        }?.template
    }
}
