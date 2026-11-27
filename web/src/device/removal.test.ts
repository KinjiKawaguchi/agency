import type { DeviceList } from '../api/clientApi'
import { isRemovable, removalPlanOf, removeChangeFor } from './removal'

const device = (id: string, publicKey: string) => ({ id, label: id, publicKey, addedAt: '2026-11-20T09:00:00Z' })
const list = (orgs: ReadonlyArray<[string, ReadonlyArray<[string, string]>]>): DeviceList => ({
  orgs: orgs.map(([org, devices]) => ({ org, devices: devices.map(([id, key]) => device(id, key)) })),
  requests: [],
})

describe('what removing a device means, org by org', () => {
  const both = list([
    ['acme', [['d-aaaa', 'KEY-A'], ['d-bbbb', 'KEY-B']]],
    ['club', [['d-aaaa', 'KEY-A'], ['d-bbbb', 'KEY-B']]],
  ])

  it('lists each org that holds the device, with its id there and the id of this browser that signs for it', () => {
    const plan = removalPlanOf(both, 'KEY-B', 'KEY-A')
    expect(plan.orgs).toEqual([
      { org: 'acme', deviceId: 'd-bbbb', signerId: 'd-aaaa', isLast: false },
      { org: 'club', deviceId: 'd-bbbb', signerId: 'd-aaaa', isLast: false },
    ])
    expect(plan.isThisDevice).toBe(false)
    expect(isRemovable(plan)).toBe(true)
  })

  it('says a removal is of this device when the key to remove is this browser’s own', () => {
    const plan = removalPlanOf(both, 'KEY-A', 'KEY-A')
    expect(plan.isThisDevice).toBe(true)
    expect(plan.signedIn.map((one) => one.signerId)).toEqual(['d-aaaa', 'd-aaaa'])
  })

  it('refuses the removal as a whole when the device is the only one in any org, and names that org', () => {
    const plan = removalPlanOf(list([['acme', [['d-aaaa', 'KEY-A'], ['d-bbbb', 'KEY-B']]], ['club', [['d-bbbb', 'KEY-B']]]]), 'KEY-B', 'KEY-A')
    expect(plan.lastIn).toEqual(['club'])
    expect(isRemovable(plan)).toBe(false)
  })

  it('does not offer to remove the only device of the account', () => {
    const plan = removalPlanOf(list([['acme', [['d-aaaa', 'KEY-A']]]]), 'KEY-A', 'KEY-A')
    expect(plan.lastIn).toEqual(['acme'])
    expect(isRemovable(plan)).toBe(false)
  })

  it('removes where this browser can sign, and names the orgs it cannot sign in, which keep the device', () => {
    const plan = removalPlanOf(list([['acme', [['d-aaaa', 'KEY-A'], ['d-bbbb', 'KEY-B']]], ['club', [['d-cccc', 'KEY-C'], ['d-bbbb', 'KEY-B']]]]), 'KEY-B', 'KEY-A')
    expect(plan.signedIn.map((one) => one.org)).toEqual(['acme'])
    expect(plan.unsignedIn).toEqual(['club'])
    expect(isRemovable(plan)).toBe(true)
  })

  it('is not removable when this browser can sign nowhere the device is listed, or the device is listed nowhere', () => {
    expect(isRemovable(removalPlanOf(list([['club', [['d-cccc', 'KEY-C'], ['d-bbbb', 'KEY-B']]]]), 'KEY-B', 'KEY-A'))).toBe(false)
    expect(isRemovable(removalPlanOf(both, 'KEY-Z', 'KEY-A'))).toBe(false)
  })
})

describe('the change that removes a device', () => {
  it('names the org, the account, the device by its id in that org, and when', () => {
    expect(removeChangeFor('acme', 'alice', 'd-bbbb', new Date('2026-11-27T12:00:00Z'))).toEqual({ org: 'acme', account: 'alice', op: 'remove', device: 'd-bbbb', at: '2026-11-27T12:00:00Z' })
  })
})
