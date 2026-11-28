// Written by RouteTemplatesTests from testdata/route-templates.json: run `AGENCY_WRITE_FIXTURES=1 swift test --filter RouteTemplatesTests` in
// apple/AgencyKit after the node's routes change, and commit the file. Do not edit it by hand.

extension RouteTemplates {
    /// The routes a client may name, in the order of `testdata/route-templates.json`.
    public static let all: [String] = [
        "DELETE /api/push/subscriptions/{id}",
        "DELETE /api/push/tokens/{token}",
        "GET /api/about",
        "GET /api/claim",
        "GET /api/claims",
        "GET /api/decisions",
        "GET /api/devices",
        "GET /api/me",
        "GET /api/metrics",
        "GET /api/orgs/{org}",
        "GET /api/orgs/{org}/effort",
        "GET /api/orgs/{org}/evaluation",
        "GET /api/orgs/{org}/history",
        "GET /api/orgs/{org}/members",
        "GET /api/orgs/{org}/outcomes",
        "GET /api/orgs/{org}/outcomes/observations",
        "GET /api/orgs/{org}/outcomes/readings",
        "GET /api/orgs/{org}/teams/{team}/activity",
        "GET /api/orgs/{org}/teams/{team}/budget",
        "GET /api/orgs/{org}/teams/{team}/usage",
        "GET /api/orgs/{org}/teams/{team}/view",
        "GET /api/push/subscriptions",
        "GET /api/push/tokens",
        "GET /api/push/vapid",
        "GET /api/session",
        "POST /api/claim",
        "POST /api/claims/{id}/confirm",
        "POST /api/claims/{id}/reject",
        "POST /api/devices/changes",
        "POST /api/devices/requests",
        "POST /api/orgs/{org}/outcomes/observations",
        "POST /api/orgs/{org}/teams/{team}/envelopes",
        "POST /api/push/subscriptions",
        "POST /api/push/tokens",
    ]
}
