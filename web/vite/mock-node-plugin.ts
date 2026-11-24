import type { IncomingMessage, ServerResponse } from 'node:http'
import type { Connect, Plugin } from 'vite'
import { createMockNode } from '../src/mock/node.ts'

async function readBody(message: IncomingMessage): Promise<Buffer> {
  const chunks: Buffer[] = []
  for await (const chunk of message) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks)
}

function middleware(): Connect.NextHandleFunction {
  const node = createMockNode()
  return async (message: IncomingMessage, response: ServerResponse, next: Connect.NextFunction) => {
    if (!message.url?.startsWith('/api/')) return next()
    const method = message.method ?? 'GET'
    const hasBody = method !== 'GET' && method !== 'HEAD'
    const request = new Request(new URL(message.url, 'http://localhost'), {
      method,
      headers: { 'content-type': message.headers['content-type'] ?? 'application/json' },
      ...(hasBody ? { body: new Uint8Array(await readBody(message)) } : {}),
    })
    const reply = await node.handle(request)
    response.statusCode = reply.status
    response.setHeader('content-type', reply.headers.get('content-type') ?? 'application/json')
    response.end(await reply.text())
  }
}

/** Serves the mock node's Client API from the dev and preview servers. */
export function mockNodePlugin(): Plugin {
  return {
    name: 'agency-mock-node',
    configureServer: (server) => void server.middlewares.use(middleware()),
    configurePreviewServer: (server) => void server.middlewares.use(middleware()),
  }
}
