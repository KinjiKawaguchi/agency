// The home nodes this browser knows about, other than the one that served the page. In a browser a home node is an
// origin: the page and its API come from the node itself, so the sign-in, the device key, and the drafts are
// already kept apart by origin. What is stored here is only a label and an address, as a link to open; nothing of
// a node's data, token, or key, and the page never fetches from another origin.

/** The longest label a home node is given. */
export const MAX_NODE_LABEL = 40
const MAX_NODES = 20
const STORAGE_KEY = 'agency.homeNodes'

export interface HomeNode {
  /** What the owner calls it. */
  label: string
  /** `https://host[:port]`, or `http://localhost[:port]` for a node on this machine. */
  origin: string
}

export interface HomeNodes {
  /** What the owner calls the node that served this page; the host name when absent. */
  self: string | undefined
  others: readonly HomeNode[]
}

export const NO_NODES: HomeNodes = { self: undefined, others: [] }

export type ParsedAddress = { ok: true; origin: string } | { ok: false; reason: 'empty' | 'not-an-address' | 'needs-https' | 'has-credentials' }

const LOOPBACK = new Set(['localhost', '127.0.0.1', '[::1]'])

/**
 * The origin of an address the owner typed: `https`, or `http` only on this machine, with no name or
 * password, and the path, query, and fragment dropped. A bare host name is read as `https`.
 */
export function parseNodeAddress(input: string): ParsedAddress {
  const text = input.trim()
  if (text === '') return { ok: false, reason: 'empty' }
  let url: URL
  try {
    url = new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `https://${text}`)
  } catch {
    return { ok: false, reason: 'not-an-address' }
  }
  if (url.username !== '' || url.password !== '') return { ok: false, reason: 'has-credentials' }
  const isLocal = url.protocol === 'http:' && LOOPBACK.has(url.hostname)
  if (url.protocol !== 'https:' && !isLocal) return { ok: false, reason: url.protocol === 'http:' ? 'needs-https' : 'not-an-address' }
  if (url.hostname === '') return { ok: false, reason: 'not-an-address' }
  return { ok: true, origin: url.origin }
}

const cleanLabel = (label: string): string => [...label.trim()].slice(0, MAX_NODE_LABEL).join('')

/** A home node the page lists: one the owner added in this browser, or one the node's configuration names (`Me.homeNodes`). */
export interface ListedNode extends HomeNode {
  isFromNode: boolean
}

/**
 * What the page lists: the nodes the node's configuration names, then the ones the owner added here. An entry is
 * read again by the page's own rule (`parseNodeAddress`), so one the page could not open is not listed; an origin
 * is listed once (the node's entry first), and this node's own origin never. The node's labels are shown whole.
 */
export function listedNodes(own: HomeNodes, fromNode: ReadonlyArray<{ label: string; url: string }> | undefined, selfOrigin: string): ListedNode[] {
  const seen = new Set<string>([selfOrigin])
  const listed: ListedNode[] = []
  const add = (label: string, address: string, isFromNode: boolean): void => {
    const parsed = parseNodeAddress(address)
    if (!parsed.ok || seen.has(parsed.origin)) return
    seen.add(parsed.origin)
    listed.push({ label: label.trim() === '' ? new URL(parsed.origin).host : label.trim(), origin: parsed.origin, isFromNode })
  }
  for (const one of fromNode ?? [] /* stand-in-ok(optional): the node leaves its list out for anyone but its owner, in an answer that has come */) add(one.label, one.url, true)
  for (const one of own.others) add(one.label, one.origin, false)
  return listed
}

/** Adds a node, or renames the one already listed at that origin. A node's own origin is not listed. */
export function withNode(nodes: HomeNodes, label: string, origin: string, selfOrigin: string): HomeNodes {
  if (origin === selfOrigin) return nodes
  const named = cleanLabel(label) === '' ? new URL(origin).host : cleanLabel(label)
  const rest = nodes.others.filter((one) => one.origin !== origin)
  return { ...nodes, others: [...rest, { label: named, origin }].slice(0, MAX_NODES) }
}

export const withoutNode = (nodes: HomeNodes, origin: string): HomeNodes => ({ ...nodes, others: nodes.others.filter((one) => one.origin !== origin) })

/** Names the node that served this page; an empty name goes back to its host name. */
export const withSelfLabel = (nodes: HomeNodes, label: string): HomeNodes => ({ ...nodes, self: cleanLabel(label) === '' ? undefined : cleanLabel(label) })

const isNode = (value: unknown): value is HomeNode => {
  if (typeof value !== 'object' || value === null) return false
  const { label, origin } = value as Record<string, unknown>
  return typeof label === 'string' && typeof origin === 'string' && parseNodeAddress(origin).ok
}

/** What storage holds, or nothing when it is missing, malformed, or not allowed. Only well-formed entries are kept. */
export function loadHomeNodes(storage: Pick<Storage, 'getItem'> | undefined = safeStorage()): HomeNodes {
  try {
    const stored: unknown = JSON.parse(storage?.getItem(STORAGE_KEY) ?? 'null')
    if (typeof stored !== 'object' || stored === null) return NO_NODES
    const { self, others } = stored as Record<string, unknown>
    const origins = new Set<string>()
    const kept = (Array.isArray(others) ? others : []).filter(isNode).flatMap((one) => {
      const parsed = parseNodeAddress(one.origin)
      if (!parsed.ok || origins.has(parsed.origin)) return []
      origins.add(parsed.origin)
      return [{ label: cleanLabel(one.label) === '' ? new URL(parsed.origin).host : cleanLabel(one.label), origin: parsed.origin }]
    })
    return { self: typeof self === 'string' && cleanLabel(self) !== '' ? cleanLabel(self) : undefined, others: kept.slice(0, MAX_NODES) }
  } catch {
    return NO_NODES
  }
}

export function saveHomeNodes(nodes: HomeNodes, storage: Pick<Storage, 'setItem'> | undefined = safeStorage()): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(nodes))
  } catch {
    // Not kept: the list lasts until the page closes.
  }
}

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage
  } catch {
    return undefined
  }
}

/** Whether the page runs in the iPhone app, whose own switcher owns the nodes and whose web view must stay where it is. */
export const isNativeApp = (page: object = window): boolean => {
  const native = (page as { agencyNative?: unknown }).agencyNative
  return typeof native === 'object' && native !== null
}
