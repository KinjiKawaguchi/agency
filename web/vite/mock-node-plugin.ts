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

function apiMiddleware(): Connect.NextHandleFunction {
  // The real node does not serve token use yet. AGENCY_MOCK_USAGE=1 turns the mock numbers on.
  const node = createMockNode({ usage: process.env.AGENCY_MOCK_USAGE === '1' })
  return async (message: IncomingMessage, response: ServerResponse, next: Connect.NextFunction) => {
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
    response.setHeader('content-type', reply.headers.get('content-type') ?? 'application/json')
    response.end(await reply.text())
  }
}

/** The real node fills the page's token into the served index.html; the mock does the same with its own. */
const withToken = (html: string): string => html.replace('__AGENCY_TOKEN__', MOCK_TOKEN)

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
        response.setHeader('content-type', 'text/html')
        response.end(withToken(html))
      })
    },
  }
}
