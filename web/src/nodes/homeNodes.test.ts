import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Me } from '../api/clientApi'
import { isNativeApp, listedNodes, loadHomeNodes, MAX_NODE_LABEL, NO_NODES, parseNodeAddress, saveHomeNodes, withNode, withoutNode, withSelfLabel, type HomeNodes } from './homeNodes'

const SELF = 'https://agency.example'

describe('parseNodeAddress', () => {
  it('takes the origin of an https address and drops what follows it', () => {
    expect(parseNodeAddress('https://work.example.com/app/desk?x=1#y')).toEqual({ ok: true, origin: 'https://work.example.com' })
    expect(parseNodeAddress('https://work.example.com:8443/')).toEqual({ ok: true, origin: 'https://work.example.com:8443' })
  })

  it('reads a bare host name as https', () => {
    expect(parseNodeAddress('  work.example.com  ')).toEqual({ ok: true, origin: 'https://work.example.com' })
  })

  it('allows http only on this machine', () => {
    expect(parseNodeAddress('http://localhost:4400')).toEqual({ ok: true, origin: 'http://localhost:4400' })
    expect(parseNodeAddress('http://127.0.0.1:4400/x')).toEqual({ ok: true, origin: 'http://127.0.0.1:4400' })
    expect(parseNodeAddress('http://work.example.com')).toEqual({ ok: false, reason: 'needs-https' })
    expect(parseNodeAddress('http://192.168.1.5:4400')).toEqual({ ok: false, reason: 'needs-https' })
  })

  it('refuses a name or a password in the address', () => {
    expect(parseNodeAddress('https://alice:secret@work.example.com')).toEqual({ ok: false, reason: 'has-credentials' })
    expect(parseNodeAddress('https://alice@work.example.com')).toEqual({ ok: false, reason: 'has-credentials' })
  })

  it('refuses what is not an address, and other schemes', () => {
    expect(parseNodeAddress('')).toEqual({ ok: false, reason: 'empty' })
    expect(parseNodeAddress('   ')).toEqual({ ok: false, reason: 'empty' })
    expect(parseNodeAddress('https://')).toEqual({ ok: false, reason: 'not-an-address' })
    expect(parseNodeAddress('javascript://example.com/%0Aalert(1)')).toEqual({ ok: false, reason: 'not-an-address' })
    expect(parseNodeAddress('ftp://work.example.com')).toEqual({ ok: false, reason: 'not-an-address' })
    expect(parseNodeAddress('not an address')).toEqual({ ok: false, reason: 'not-an-address' })
  })
})

describe('the list', () => {
  it('adds a node and names it by its host when no label is given', () => {
    const nodes = withNode(NO_NODES, '  ', 'https://work.example.com', SELF)
    expect(nodes.others).toEqual([{ label: 'work.example.com', origin: 'https://work.example.com' }])
  })

  it('renames the node already listed at an origin instead of listing it twice', () => {
    const once = withNode(NO_NODES, 'Work', 'https://work.example.com', SELF)
    expect(withNode(once, 'Office', 'https://work.example.com', SELF).others).toEqual([{ label: 'Office', origin: 'https://work.example.com' }])
  })

  it('does not list the node that served the page', () => {
    expect(withNode(NO_NODES, 'Me', SELF, SELF)).toBe(NO_NODES)
  })

  it('cuts a long label, by characters', () => {
    const label = '作'.repeat(MAX_NODE_LABEL + 5)
    expect([...(withNode(NO_NODES, label, 'https://work.example.com', SELF).others[0]?.label ?? '')]).toHaveLength(MAX_NODE_LABEL)
  })

  it('removes a node by its origin and leaves the others', () => {
    const two = withNode(withNode(NO_NODES, 'A', 'https://a.example', SELF), 'B', 'https://b.example', SELF)
    expect(withoutNode(two, 'https://a.example').others.map((one) => one.label)).toEqual(['B'])
  })

  it('names this node, and an empty name goes back to the host name', () => {
    const named = withSelfLabel(NO_NODES, ' Personal ')
    expect(named.self).toBe('Personal')
    expect(withSelfLabel(named, '  ').self).toBeUndefined()
  })
})

describe('storage', () => {
  const memory = (initial?: string) => {
    let value = initial
    return { getItem: () => value ?? null, setItem: (_key: string, next: string) => void (value = next) }
  }

  it('keeps the list and reads it back', () => {
    const storage = memory()
    const nodes: HomeNodes = { self: 'Personal', others: [{ label: 'Work', origin: 'https://work.example.com' }] }
    saveHomeNodes(nodes, storage)
    expect(loadHomeNodes(storage)).toEqual(nodes)
  })

  it('reads nothing from what is missing or malformed', () => {
    expect(loadHomeNodes(memory())).toEqual(NO_NODES)
    expect(loadHomeNodes(memory('not json'))).toEqual(NO_NODES)
    expect(loadHomeNodes(memory('42'))).toEqual(NO_NODES)
    expect(loadHomeNodes(undefined)).toEqual(NO_NODES)
  })

  it('keeps only entries that are well formed, once each, and never an address the form would refuse', () => {
    const stored = JSON.stringify({
      self: 7,
      others: [
        { label: 'Work', origin: 'https://work.example.com/path' },
        { label: 'Again', origin: 'https://work.example.com' },
        { label: 'Plain http', origin: 'http://work.example.com' },
        { label: 'Script', origin: 'javascript:alert(1)' },
        { label: 3, origin: 'https://other.example.com' },
        'text',
      ],
    })
    expect(loadHomeNodes(memory(stored))).toEqual({ self: undefined, others: [{ label: 'Work', origin: 'https://work.example.com' }] })
  })

  it('does not fail when storage is not allowed to write', () => {
    expect(() => saveHomeNodes(NO_NODES, { setItem: () => { throw new Error('full') } })).not.toThrow()
  })
})

describe('isNativeApp', () => {
  it('is true only where the app has set its hand-over object', () => {
    expect(isNativeApp({ agencyNative: { platform: 'ios' } })).toBe(true)
    expect(isNativeApp({})).toBe(false)
    expect(isNativeApp({ agencyNative: null })).toBe(false)
    expect(isNativeApp({ agencyNative: 'yes' })).toBe(false)
  })
})

describe('listedNodes', () => {
  const own = (...entries: Array<[string, string]>): HomeNodes => ({ self: undefined, others: entries.map(([label, origin]) => ({ label, origin })) })

  it('lists the nodes the node names first, then the owner’s own, each marked by where it comes from', () => {
    const listed = listedNodes(own(['Lab', 'https://lab.example.com']), [{ label: 'Work', url: 'https://work.example.com' }], SELF)
    expect(listed).toEqual([
      { label: 'Work', origin: 'https://work.example.com', isFromNode: true },
      { label: 'Lab', origin: 'https://lab.example.com', isFromNode: false },
    ])
  })

  it('lists only the owner’s own when the node names none, as for anyone but its owner', () => {
    expect(listedNodes(own(['Lab', 'https://lab.example.com']), undefined, SELF)).toEqual([{ label: 'Lab', origin: 'https://lab.example.com', isFromNode: false }])
    expect(listedNodes(NO_NODES, [], SELF)).toEqual([])
  })

  it('lists an origin once, the node’s entry first, whatever the spelling of the other', () => {
    const listed = listedNodes(own(['My work', 'https://WORK.example.com:443/desk']), [{ label: 'Work', url: 'https://work.example.com' }], SELF)
    expect(listed).toEqual([{ label: 'Work', origin: 'https://work.example.com', isFromNode: true }])
  })

  it('never lists this node, however the node wrote it', () => {
    expect(listedNodes(own(['Me', SELF]), [{ label: 'Me', url: SELF }], SELF)).toEqual([])
  })

  it('leaves out an entry this page could not open, which the node’s own rule should have kept out', () => {
    const listed = listedNodes(NO_NODES, [{ label: 'Bad', url: 'http://work.example.com' }, { label: 'Odd', url: 'https://a[b]' }, { label: 'Fine', url: 'https://ok.example.com' }], SELF)
    expect(listed.map((one) => one.label)).toEqual(['Fine'])
  })

  it('shows a node’s label whole (the node allows 80 characters), and names a node with an empty label by its host', () => {
    const long = 'x'.repeat(80)
    expect(listedNodes(NO_NODES, [{ label: long, url: 'https://long.example.com' }], SELF)[0]?.label).toBe(long)
    expect(listedNodes(NO_NODES, [{ label: '  ', url: 'https://work.example.com:8443' }], SELF)[0]?.label).toBe('work.example.com:8443')
  })

  it('reads the node’s answer in testdata: an https name and a node on the loopback', () => {
    const me = JSON.parse(readFileSync(join(process.cwd(), '..', 'testdata', 'client', 'me-home-nodes.json'), 'utf8')) as Me
    expect(listedNodes(NO_NODES, me.homeNodes, 'http://localhost:4173').map((one) => [one.label, one.origin, one.isFromNode])).toEqual([
      ['Work', 'https://work.example.com', true],
      ['Lab', 'http://127.0.0.1:4500', true],
    ])
  })
})
