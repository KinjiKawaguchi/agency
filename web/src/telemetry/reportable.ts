// The routes a client may name in a report of how long its calls took: the node's own table of them (`testdata/route-templates.json`, which the
// node holds to its router and the apps read too), so that the page keeps no second list. A sample of a route not in it would only be dropped
// and counted by the node, so it is not sent.
import table from '../../../testdata/route-templates.json' with { type: 'json' }

export const REPORTABLE_ROUTES: ReadonlySet<string> = new Set(table.routes)

export const isReportable = (route: string): boolean => REPORTABLE_ROUTES.has(route)
