import { createApiClient, type ApiClient } from '../api/client'
import { createMockNode } from '../mock/node'

interface Flaw {
  /** Which reads it spoils: the part of the address, or all of it with `exact`. */
  match: string
  exact?: boolean
  /** How many of them fail before they pass; all of them when absent. */
  failures?: number
  status?: number
}

/** A mock node whose reads of some addresses fail with a status (503 by default) a few times, or always, and a count of the reads. */
export function flakyNode(flaws: readonly Flaw[]): { client: ApiClient; reads: (match: string) => number } {
  const node = createMockNode({ moreOrgs: false })
  const seen = new Map<string, number>()
  const client = createApiClient(async (request) => {
    const url = new URL(request.url, 'http://mock.local')
    for (const flaw of flaws) {
      if (flaw.exact === true ? url.pathname !== flaw.match : !url.pathname.includes(flaw.match)) continue
      const before = seen.get(flaw.match) ?? 0
      seen.set(flaw.match, before + 1)
      if (flaw.failures === undefined || before < flaw.failures) {
        return new Response(JSON.stringify({ error: 'The board has not been read yet; a refresh was started.' }), { status: flaw.status ?? 503, headers: { 'content-type': 'application/json' } })
      }
    }
    return node.handle(request)
  })
  return { client, reads: (match) => seen.get(match) ?? 0 }
}
