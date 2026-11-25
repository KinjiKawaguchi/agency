import { badgeOf, MAX_BADGE, setNativeBadge } from './badge'

describe('badgeOf', () => {
  it('keeps a whole number from 0 to 9999, which is all the app takes', () => {
    expect([0, 3, 9999].map(badgeOf)).toEqual([0, 3, 9999])
    expect(badgeOf(10_000)).toBe(MAX_BADGE)
    expect(badgeOf(-4)).toBe(0)
    expect(badgeOf(2.9)).toBe(2)
  })
})

describe('setNativeBadge', () => {
  it('calls the app’s setBadge with the number, on the object the app set', () => {
    const calls: Array<[unknown, number]> = []
    const native = {
      setBadge(this: unknown, count: number) {
        calls.push([this, count])
      },
    }
    setNativeBadge({ agencyNative: native }, 12_345)
    expect(calls).toEqual([[native, 9999]])
  })

  it('does nothing in a browser, or in an app without it', () => {
    expect(() => setNativeBadge({}, 3)).not.toThrow()
    expect(() => setNativeBadge({ agencyNative: { platform: 'ios', push: null } }, 3)).not.toThrow()
    expect(() => setNativeBadge({ agencyNative: { setBadge: 'not a function' } }, 3)).not.toThrow()
  })

  it('does not let a failing bridge take the page down', () => {
    expect(() => setNativeBadge({ agencyNative: { setBadge: () => { throw new Error('no handler') } } }, 3)).not.toThrow()
  })
})
