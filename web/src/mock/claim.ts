import type { ClaimFailure, ClaimInput, ClaimState } from '../claim/provisional'
import { pairingCodeOf } from '../device/fingerprint'

/** A claim link the mock node has made: the code of one seat, and how a redeem of it ends. */
export interface MockClaimOptions {
  /** The code in the link; a call with another is "unknown or used". */
  code: string
  seat?: string
  org?: string
  /** Makes a redeem of the right code end in this refusal, as the node's statuses say it. */
  failWith?: ClaimFailure
  /** The node answers this pairing code instead of the one of the key it was sent: a node that holds another key. */
  answersCode?: string
}

const STATUS: Record<ClaimFailure, [number, string]> = {
  unknown: [404, 'That link is not valid or was already used.'],
  expired: [410, 'That link has expired.'],
  'other-email': [403, 'That link was made for another address.'],
  taken: [409, 'That seat is already taken.'],
}

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
const failure = (kind: ClaimFailure): Response => json({ error: STATUS[kind][1] }, STATUS[kind][0])

/** The node's claim routes for one seat, from the design on agency#196: a redeem once, then the state until the owner confirms. */
export function createMockClaim(options: MockClaimOptions | undefined) {
  let claimed: ClaimState | undefined
  let isUsed = false
  return {
    /** What the owner does at the node: compares the pairing code and confirms. */
    confirm: (): void => {
      if (claimed !== undefined) claimed = { ...claimed, state: 'in' }
    },
    state: (): ClaimState | undefined => claimed,
    /** The node comes to answer another pairing code than before: it holds another key than the one it was sent. */
    holdOtherKey: (pairingCode: string): void => {
      if (claimed !== undefined) claimed = { ...claimed, pairingCode }
    },
    /** Answers a claim route, or nothing when the request is not one. `write` is the node's check of the write token. */
    handle(request: Request, url: URL, write: () => Promise<Response | { body: unknown }>): Promise<Response> | undefined {
      if (url.pathname !== '/api/claim') return undefined
      if (options === undefined) return Promise.resolve(json({ error: 'No such route.' }, 404))
      if (request.method === 'GET') return Promise.resolve(claimed === undefined ? failure('unknown') : json(claimed))
      if (request.method !== 'POST') return undefined
      return write().then(async (answer) => {
        if (answer instanceof Response) return answer
        const input = answer.body as Partial<ClaimInput> | null
        if (input === null || typeof input.code !== 'string' || typeof input.publicKey !== 'string' || input.code !== options.code || isUsed) return failure('unknown')
        if (options.failWith !== undefined) return failure(options.failWith)
        isUsed = true
        claimed = { state: 'waiting', seat: options.seat ?? 'Carol (finance)', org: options.org ?? 'Acme', pairingCode: options.answersCode ?? (await pairingCodeOf(input.publicKey)) }
        return json(claimed)
      })
    },
  }
}
