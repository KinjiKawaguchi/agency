import Foundation

/// The routes a client may name when it reports what it observed (`POST /api/telemetry`), as `testdata/route-templates.json` lists them: a
/// route is a method and a path whose ids are `{name}` (`GET /api/orgs/{org}/teams/{team}/view`). A call to anything else is not reported.
public enum RouteTemplates {
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
