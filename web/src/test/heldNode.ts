import { createApiClient, type ApiClient } from '../api/client'
import type { MockNode } from '../mock/node'

/**
 * A client for a mock node that holds the reads of some addresses (the part of the path, or a pattern for all of it) until they are released,
 * so a test can look at a screen in the time before the node has answered, and then at the answer.
 */
export function heldNode(node: MockNode, holds: readonly (string | RegExp)[]): { client: ApiClient; release: (match?: string | RegExp) => void } {
  const waiting = new Map<string | RegExp, Array<() => void>>()
  const released = new Set<string | RegExp>()
  const client = createApiClient(async (request) => {
    const path = new URL(request.url, 'http://mock.local').pathname
    const match = holds.find((one) => (typeof one === 'string' ? path.includes(one) : one.test(path)))
    if (match !== undefined && request.method === 'GET' && !released.has(match)) {
      await new Promise<void>((resolve) => waiting.set(match, [...(waiting.get(match) ?? []), resolve]))
    }
    return node.handle(request)
  })
  const release = (match?: string | RegExp): void => {
    for (const one of match === undefined ? holds : [match]) {
      released.add(one)
      for (const resolve of waiting.get(one) ?? []) resolve()
      waiting.delete(one)
    }
  }
  return { client, release }
}
