import { pushOfPage, readPush } from './push'

const GOOD = { token: 'ab'.repeat(32), environment: 'sandbox', label: 'iPhone' }

describe('readPush', () => {
  it('takes the shape the app promises', () => {
    expect(readPush(GOOD)).toEqual(GOOD)
    expect(readPush({ ...GOOD, token: 'ab'.repeat(16) })).toBeDefined()
    expect(readPush({ ...GOOD, token: 'ab'.repeat(100) })).toBeDefined()
    expect(readPush({ ...GOOD, environment: 'production', label: ' iPhone ' })).toEqual({ ...GOOD, environment: 'production', label: 'iPhone' })
  })

  it.each([
    ['null', null],
    ['not an object', 'abcd'],
    ['a token that is not hex', { ...GOOD, token: 'xyz' }],
    ['a token in capitals', { ...GOOD, token: 'ABCD' }],
    ['an empty token', { ...GOOD, token: '' }],
    ['a token that is too short', { ...GOOD, token: 'ab'.repeat(15) }],
    ['a token that is too long', { ...GOOD, token: 'ab'.repeat(101) }],
    ['a token that is not a string', { ...GOOD, token: 12 }],
    ['an unknown environment', { ...GOOD, environment: 'staging' }],
    ['no label', { ...GOOD, label: ' ' }],
    ['a label that is too long', { ...GOOD, label: 'x'.repeat(81) }],
  ])('refuses %s', (_name, value) => {
    expect(readPush(value)).toBeUndefined()
  })
})

describe('pushOfPage', () => {
  it('reads what the app set, and nothing outside the app', () => {
    expect(pushOfPage({ agencyNative: { platform: 'ios', push: GOOD } })).toEqual(GOOD)
    expect(pushOfPage({ agencyNative: { platform: 'ios', push: null } })).toBeUndefined()
    expect(pushOfPage({ agencyNative: { platform: 'ios' } })).toBeUndefined()
    expect(pushOfPage({})).toBeUndefined()
  })
})
