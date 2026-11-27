// A call of the Client API, named by its route template as the node names it (`GET /api/orgs/{org}/teams/{team}/view`) and given its
// path from that same template, so the two cannot drift: the template is what the page reports of how long a call took (never a
// path with an org or a team in it), and the path is what is asked. The node's own list of templates is testdata/route-templates.json.

/** A call: its route template, with the method, and the path with the values in it. */
export interface Target<Route extends string = string> {
  route: Route
  path: string
}

type Value = string | number

/**
 * The target of a route template with the values of its `{name}` parts (each written as one path segment) and an optional query. A part
 * with no value, a value with no part, and a template that does not start with a method and `/api/` are mistakes of the code and throw.
 */
export function at<Route extends `${'GET' | 'POST' | 'DELETE'} /api/${string}`>(route: Route, values: Readonly<Record<string, Value>> = {}, query: Readonly<Record<string, Value | undefined>> = {}): Target<Route> {
  const template = route.slice(route.indexOf(' ') + 1)
  const wanted = [...template.matchAll(/\{([a-z]+)\}/g)].map((found) => found[1] ?? '' /* stand-in-ok(index): a part found by the pattern has its name */)
  const given = Object.keys(values)
  if (wanted.length !== given.length || wanted.some((name) => !Object.hasOwn(values, name))) throw new Error(`${route} takes {${wanted.join('}, {')}} and was given ${given.join(', ') || 'nothing'}`)
  const path = template.replace(/\{([a-z]+)\}/g, (_part, name: string) => encodeURIComponent(String(values[name])))
  const pairs = Object.entries(query).flatMap(([key, value]) => (value === undefined ? [] : [`${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`]))
  return { route, path: pairs.length === 0 ? path : `${path}?${pairs.join('&')}` }
}
