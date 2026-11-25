import { readFile } from 'node:fs/promises'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { resolve } from 'node:path'
import type { Connect, Plugin } from 'vite'
import { createMockNode, MOCK_TOKEN } from '../src/mock/node.ts'

async function readBody(message: IncomingMessage): Promise<Buffer> {
  const chunks: Buffer[] = []
  for await (const chunk of message) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks)
}

const RESET_PATH = '/__mock/reset'
const SIGN_IN_PATH = '/__mock/sign-in'
const DEVICE_ADD_PATH = '/__mock/device-add'
const ENVELOPES_PATH = '/__mock/envelopes'
const PUSH_TOKENS_PATH = '/__mock/push-tokens'

/**
 * Options for a fresh mock node. The real node does not serve token use yet, so AGENCY_MOCK_USAGE=1 turns
 * the mock numbers on; `POST /__mock/reset?usage=1&siteError=...&outcome=queued` starts a node with the given ones.
 */
function nodeOptions(query: URLSearchParams | undefined): Parameters<typeof createMockNode>[0] {
  const usage = query?.get('usage') ?? process.env.AGENCY_MOCK_USAGE
  const siteError = query?.get('siteError')
  const outcome = query?.get('outcome')
  const isEmpty = query?.get('empty') === '1'
  return {
    usage: usage === '1',
    ...(query?.get('orgs') === 'more' ? { moreOrgs: true } : {}),
    ...(isEmpty ? { nothingWaiting: true, earlierDecisions: false } : {}),
    ...(siteError === null || siteError === undefined ? {} : { siteError }),
    ...(outcome === 'queued' || outcome === 'delivered' ? { outcome } : {}),
  }
}

function apiMiddleware(): Connect.NextHandleFunction {
  let node = createMockNode(nodeOptions(undefined))
  return async (message: IncomingMessage, response: ServerResponse, next: Connect.NextFunction) => {
    const url = new URL(message.url ?? '/', 'http://localhost')
    if (message.method === 'POST' && url.pathname === RESET_PATH) {
      node = createMockNode(nodeOptions(url.searchParams))
      response.statusCode = 204
      return response.end()
    }
    if (message.method === 'POST' && url.pathname === SIGN_IN_PATH) {
      node.setSignInExpired(url.searchParams.get('expired') === '1')
      response.statusCode = 204
      return response.end()
    }
    if (message.method === 'POST' && url.pathname === DEVICE_ADD_PATH) {
      response.statusCode = node.addDeviceFromTerminal(url.searchParams.get('code') ?? '') ? 204 : 404
      return response.end()
    }
    if (message.method === 'GET' && url.pathname === ENVELOPES_PATH) {
      response.setHeader('content-type', 'application/json')
      return response.end(JSON.stringify(node.envelopes()))
    }
    if (message.method === 'GET' && url.pathname === PUSH_TOKENS_PATH) {
      response.setHeader('content-type', 'application/json')
      return response.end(JSON.stringify(node.pushTokens()))
    }
    if (!message.url?.startsWith('/api/')) return next()
    const method = message.method ?? 'GET'
    const hasBody = method !== 'GET' && method !== 'HEAD'
    const headers = new Headers()
    for (const name of ['content-type', 'x-agency-token']) {
      const value = message.headers[name]
      if (typeof value === 'string') headers.set(name, value)
    }
    const request = new Request(new URL(message.url, 'http://localhost'), { method, headers, ...(hasBody ? { body: new Uint8Array(await readBody(message)) } : {}) })
    const reply = await node.handle(request)
    response.statusCode = reply.status
    const location = reply.headers.get('location')
    if (location !== null) response.setHeader('location', location)
    response.setHeader('content-type', reply.headers.get('content-type') ?? 'application/json')
    response.end(await reply.text())
  }
}

/** The real node fills the page's token into the served index.html; the mock does the same with its own. */
const withToken = (html: string): string => html.replace('__AGENCY_TOKEN__', MOCK_TOKEN)

/** The headers the real node puts on every page (docs/protocol.md, "Who is let in"), so the built client is exercised under them. */
const PAGE_HEADERS: Record<string, string> = {
  'content-type': 'text/html',
  'content-security-policy': "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
  'x-content-type-options': 'nosniff',
  'referrer-policy': 'no-referrer',
  'cache-control': 'no-store',
}

const isPageRequest = (message: IncomingMessage): boolean =>
  message.method === 'GET' && message.url !== undefined && !message.url.startsWith('/api/') && !/\.[a-z0-9]+$/i.test(message.url.split('?')[0] ?? '')

/** Serves the mock node's Client API from the dev and preview servers, and its token in the page. */
export function mockNodePlugin(): Plugin {
  return {
    name: 'agency-mock-node',
    configureServer: (server) => void server.middlewares.use(apiMiddleware()),
    transformIndexHtml: { order: 'post', handler: (html, context) => (context.server === undefined ? html : withToken(html)) },
    configurePreviewServer: (server) => {
      server.middlewares.use(apiMiddleware())
      server.middlewares.use(async (message, response, next) => {
        if (!isPageRequest(message)) return next()
        const html = await readFile(resolve(server.config.root, server.config.build.outDir, 'index.html'), 'utf8')
        for (const [name, value] of Object.entries(PAGE_HEADERS)) response.setHeader(name, value)
        response.end(withToken(html))
      })
    },
  }
}
