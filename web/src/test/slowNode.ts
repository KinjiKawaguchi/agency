import { createApiClient, type ApiClient } from '../api/client'
import type { createMockNode } from '../mock/node'

/**
 * A client for a mock node whose list of devices is slow to answer once this browser has asked to be added.
 * The screen has to wait for that list to show the request as waiting, so a test that does not wait for it
 * fails here every time instead of once in a while on a busy machine.
 */
export function slowDeviceListAfterRequest(node: ReturnType<typeof createMockNode>, ms = 300): ApiClient {
  let asked = false
  return createApiClient(async (request) => {
    const path = new URL(request.url, 'http://mock.local').pathname
    if (request.method === 'POST' && path === '/api/devices/requests') asked = true
    const answer = await node.handle(request)
    if (asked && request.method === 'GET' && path === '/api/devices') await new Promise((resolve) => setTimeout(resolve, ms))
    return answer
  })
}
