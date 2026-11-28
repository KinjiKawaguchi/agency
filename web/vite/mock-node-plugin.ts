import type { ClaimFailure } from '../src/claim/provisional'
import { readFile } from 'node:fs/promises'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { resolve } from 'node:path'
import type { Connect, Plugin } from 'vite'
import { budgetFor } from '../src/mock/budget.ts'
import { createMockNode, MOCK_TOKEN } from '../src/mock/node.ts'

async function readBody(message: IncomingMessage): Promise<Buffer> {
  const chunks: Buffer[] = []
  for await (const chunk of message) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks)
}

const RESET_PATH = '/__mock/reset'
const SIGN_IN_PATH = '/__mock/sign-in'
const CLAIM_CONFIRM_PATH = '/__mock/claim-confirm'
const DEVICE_ADD_PATH = '/__mock/device-add'
const ENVELOPES_PATH = '/__mock/envelopes'
const PUSH_TOKENS_PATH = '/__mock/push-tokens'

/**
 * Options for a fresh mock node. The real node does not serve token use yet, so AGENCY_MOCK_USAGE=1 turns
 * the mock numbers on, and AGENCY_MOCK_EVALUATION=1 does the same for the evaluation; `POST /__mock/reset?usage=1&siteError=...&outcome=queued` starts a node with the given ones.
 */
const isClaimFailure = (value: string | null | undefined): boolean => value === 'unknown' || value === 'expired' || value === 'other-email' || value === 'taken'

function nodeOptions(query: URLSearchParams | undefined): Parameters<typeof createMockNode>[0] {
  const usage = query?.get('usage') ?? process.env.AGENCY_MOCK_USAGE
  const evaluation = query?.get('evaluation') ?? process.env.AGENCY_MOCK_EVALUATION
  const activity = query?.get('activity') ?? process.env.AGENCY_MOCK_ACTIVITY
  const outcomes = query?.get('outcomes') ?? process.env.AGENCY_MOCK_OUTCOMES
  const webPush = query?.get('webPush') ?? process.env.AGENCY_MOCK_WEBPUSH
  const metrics = query?.get('metrics') ?? process.env.AGENCY_MOCK_METRICS
  const webPushFailing = query?.get('webPushFailing') ?? process.env.AGENCY_MOCK_WEBPUSH_FAILING
  const participants = query?.get('participants') ?? process.env.AGENCY_MOCK_PARTICIPANTS
  const siteError = query?.get('siteError')
  const outcome = query?.get('outcome')
  const isEmpty = query?.get('empty') === '1'
  const guest = query?.get('guest') ?? ''
  return {
    usage: usage === '1',
    evaluation: evaluation === '1',
    // The weekly budget is the node's answer, which the evaluation screen shows beside the effort; `budgetState` fixes where the app team stands (for the screenshots).
    ...(evaluation === '1' ? { budget: budgetFor(query?.get('budgetState')) } : {}),
    ...(Number.isInteger(Number(query?.get('history'))) && Number(query?.get('history')) > 0 ? { history: Number(query?.get('history')) } : {}),
    activity: activity === '1',
    participants: participants === '1',
    outcomes: outcomes === '1',
    // The owner's sample numbers are of fixed days; the cost beside them is drawn over the same days. This pins the day of the whole
    // mock node (the desk's times too), for the dev server and the screenshots.
    ...(outcomes === '1' ? { now: new Date('2026-11-24T12:00:00Z'), evaluation: true } : {}),
    webPush: webPush === '1',
    ...(metrics === 'usual' || metrics === 'empty' || metrics === 'slow' || metrics === 'absent' ? { metrics } : {}),
    webPushFailing: webPushFailing === '1',
    ...(query?.get('orgs') === 'more' ? { moreOrgs: true } : {}),
    // A claim link for one seat (agency#201): its code, and how a claim of it ends.
    ...(query?.get('claim') === null || query?.get('claim') === undefined ? {} : { claim: { code: query.get('claim') ?? '', ...(isClaimFailure(query.get('claimFails')) ? { failWith: query.get('claimFails') as ClaimFailure } : {}) } }),
    // A recovery of the org that was started and waits, as the node says it on `GET /api/me`.
    ...(query?.get('recovery') === '1' ? { recoveries: { acme: { takesEffectAt: new Date(Date.now() + 71 * 3_600_000).toISOString(), newRootKey: 'b7c8d9e0f1a2', replacesRecoveryKey: true } } } : {}),
    // A request addressed to a seat the viewer answers for (its label, as the owner wrote it).
    ...(query?.get('relayedFor') === '1' ? { relayedFor: { 'r-20261124-113000-c3d4': { id: 'finance', label: 'Erin (outside counsel)' } } } : {}),
    // The root key's node holds a device change for its owner's `agency root-sign`.
    ...(query?.get('pendingRoot') === '1' ? { pendingRoot: true } : {}),
    // Changes that wait for the root key's signature, as the node tells its owner.
    ...(query?.get('rootChanges') === '1'
      ? { rootChanges: [{ id: 'c1', org: 'acme', what: 'add member carol (Carol)', kind: 'addMember', member: 'carol', label: 'Carol', receivedAt: new Date(Date.now() - 2 * 3_600_000).toISOString() }, { id: 'c2', org: 'acme', what: 'add device iPhone for bob', kind: 'addDevice', member: 'bob', label: 'iPhone', receivedAt: new Date(Date.now() - 5 * 60_000).toISOString() }] }
      : {}),
    // The other home nodes the node's configuration names, as it tells its owner.
    ...(query?.get('homeNodes') === '1' ? { homeNodes: [{ label: 'Work', url: 'https://work.example.com' }, { label: 'Lab (on this machine)', url: 'http://127.0.0.1:4500' }] } : {}),
    // A second device the account has, to remove: the device key of testdata/vectors.json (its pairing code is dbc2-9825-1c51).
    ...(query?.get('laptop') === '1' ? { devices: [{ id: 'd-dbc298251c51', label: 'Work laptop', publicKey: '/RckOFqgx1tk+3jNYC+h2ZH96/drE8WO1wLqyDXp9hg=' }] } : {}),
    // A node that ties push registrations to devices (the capability `pushDevice`).
    ...(query?.get('pushDevice') === '1' ? { pushDevice: true } : {}),
    // A member the node hosts, as a guest: `guest=carol`. Only a made-up id and its label.
    ...(guest === '' ? {} : { guest: { member: guest, label: `${guest.charAt(0).toUpperCase()}${guest.slice(1)}` } }),
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
    if (message.method === 'POST' && url.pathname === CLAIM_CONFIRM_PATH) {
      node.claim.confirm()
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
