import type { DeviceList } from '../api/clientApi'
import { approvalOf, nodeCommandOf } from './approval'

const device = (id: string, label: string, publicKey: string) => ({ id, label, publicKey, addedAt: '2026-11-24T12:00:00Z' })
const MINE = 'M'.repeat(43) + '='
const list = (orgs: DeviceList['orgs'], extra: object = {}): DeviceList => ({ orgs, requests: [], ...extra }) as DeviceList

describe('approvalOf', () => {
  it('says the first device is approved on the node’s machine when the account has no device', () => {
    expect(approvalOf(list([{ org: 'acme', devices: [] }]), MINE)).toEqual({ deviceOrgs: [], deviceLabels: [], nodeOrgs: ['acme'] })
  })

  it('says a later device is approved from the devices the account has, named once each', () => {
    const laptop = device('d-1', 'Laptop', 'L'.repeat(43) + '=')
    const approval = approvalOf(list([{ org: 'acme', devices: [laptop] }, { org: 'lab', devices: [laptop, device('d-2', 'Phone', 'P'.repeat(43) + '=')] }]), MINE)
    expect(approval).toEqual({ deviceOrgs: ['acme', 'lab'], deviceLabels: ['Laptop', 'Phone'], nodeOrgs: [] })
  })

  it('splits the orgs when only some have a device', () => {
    const laptop = device('d-1', 'Laptop', 'L'.repeat(43) + '=')
    expect(approvalOf(list([{ org: 'acme', devices: [laptop] }, { org: 'lab', devices: [] }]), MINE)).toEqual({ deviceOrgs: ['acme'], deviceLabels: ['Laptop'], nodeOrgs: ['lab'] })
  })

  it('leaves out an org that already holds this device, since nothing is asked there', () => {
    const me = device('d-9', 'This one', MINE)
    expect(approvalOf(list([{ org: 'acme', devices: [me] }, { org: 'lab', devices: [] }]), MINE)).toEqual({ deviceOrgs: [], deviceLabels: [], nodeOrgs: ['lab'] })
  })
})

describe('nodeCommandOf', () => {
  const command = (value: unknown) => nodeCommandOf(list([], { node: { url: 'http://127.0.0.1:4400', dataDir: '/var/agency', command: value } }))

  it('reads the command the node wrote for itself', () => {
    expect(command('/usr/local/bin/agency --dir /var/agency device-add')).toBe('/usr/local/bin/agency --dir /var/agency device-add')
    expect(command("'/Users/some one/bin/agency' --dir '/data dir' device-add")).toBe("'/Users/some one/bin/agency' --dir '/data dir' device-add")
  })

  it('has none when the node left it out, as it does for anyone but its owner', () => {
    expect(nodeCommandOf(list([]))).toBeUndefined()
    expect(nodeCommandOf(list([], { node: null }))).toBeUndefined()
    expect(nodeCommandOf(list([], { node: {} }))).toBeUndefined()
  })

  it('refuses what is not a plain, bounded line of text', () => {
    expect(command('')).toBeUndefined()
    expect(command('   ')).toBeUndefined()
    expect(command(42)).toBeUndefined()
    expect(command('agency device-add\nrm -rf /')).toBeUndefined()
    expect(command('a'.repeat(501))).toBeUndefined()
  })
})
