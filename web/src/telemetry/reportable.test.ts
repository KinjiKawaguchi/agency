import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { isReportable, REPORTABLE_ROUTES } from './reportable'

const sourcesOf = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return sourcesOf(path)
    return /\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name) ? [path] : []
  })

/** Every route template the client names where it makes a call (`at('GET /api/...')`, or a constant that holds one). */
const usedTemplates = (): Set<string> => {
  const found = new Set<string>()
  for (const path of sourcesOf(join(process.cwd(), 'src'))) {
    const source = readFileSync(path, 'utf8')
    for (const match of source.matchAll(/at\(\s*'((?:GET|POST|DELETE) \/api\/[^']+)'/g)) found.add(match[1] ?? '')
    for (const match of source.matchAll(/const \w+ = '((?:GET|POST|DELETE) \/api\/[^']+)'/g)) found.add(match[1] ?? '')
  }
  return found
}

/**
 * Routes the client calls that a report may not name, and why. A call of a route that is not in the node's table is not reported (the node would
 * only drop and count it), so each one is a decision, written here, and not a route forgotten.
 */
const NOT_REPORTED: Readonly<Record<string, string>> = {
  'POST /api/telemetry': 'the report itself is never timed',
  'GET /api/claim': 'a person claiming a seat has none yet, and is not the owner',
  'POST /api/claim': 'a person claiming a seat has none yet, and is not the owner',
  'GET /api/orgs/{org}/effort': 'the node does not serve it yet (the screen reads the mock until it does): when the node has the route and its table, take it out of this list',
}

describe('the routes the page may name in a report', () => {
  it('has every route template the client calls, but those that are left out here on purpose', () => {
    const unlisted = [...usedTemplates()].filter((route) => !isReportable(route) && NOT_REPORTED[route] === undefined)
    expect(unlisted).toEqual([])
  })

  it('does not leave out a route that the table has, or that the client does not call, so that the list of what is left out stays true', () => {
    const used = usedTemplates()
    const stale = Object.keys(NOT_REPORTED).filter((route) => isReportable(route) || !used.has(route))
    expect(stale).toEqual([])
  })

  it('is the node’s table: methods and `{part}` templates, no path with a value in it', () => {
    expect(REPORTABLE_ROUTES.size).toBeGreaterThan(10)
    for (const route of REPORTABLE_ROUTES) expect(route).toMatch(/^(GET|POST|DELETE) \/api\/[a-z/{}-]+$/)
  })
})
