import { codeFromHash, forgetClaimCode, takeClaimCode } from './claimCode'

const CODE = 'k3JfA9xQ2mVb7LzP'

const goTo = (address: string): void => window.history.replaceState(null, '', address)

describe('the code of a claim link', () => {
  beforeEach(() => {
    forgetClaimCode()
    goTo('/')
  })

  it('is read from the fragment, and only when it is shaped like one', () => {
    expect(codeFromHash(`#code=${CODE}`)).toBe(CODE)
    expect(codeFromHash(`code=${CODE}`)).toBe(CODE)
    expect(codeFromHash(`#other=1&code=${CODE}`)).toBe(CODE)
    for (const refused of ['', '#', '#code=', '#code=short', '#code=has space in it', `#code=${'a'.repeat(201)}`, '#code=<script>alert(1)</script>', `#kode=${CODE}`]) {
      expect(codeFromHash(refused), refused).toBeUndefined()
    }
  })

  it('is taken from the address, which loses its fragment at once, with the path and the query kept', () => {
    goTo(`/claim/continue?x=1#code=${CODE}`)
    expect(takeClaimCode()).toBe(CODE)
    expect(window.location.hash).toBe('')
    expect(window.location.pathname + window.location.search).toBe('/claim/continue?x=1')
  })

  it('takes a fragment that is not a code out of the address too, and gives no code', () => {
    goTo('/claim/continue#code=nope')
    expect(takeClaimCode()).toBeUndefined()
    expect(window.location.hash).toBe('')
  })

  it('is read from this tab’s session storage when the address has none, where the landing page left it', () => {
    window.sessionStorage.setItem('agency.claimCode', CODE)
    expect(takeClaimCode()).toBe(CODE)
  })

  it('ignores what the storage holds when it is not shaped like a code', () => {
    window.sessionStorage.setItem('agency.claimCode', 'not a code')
    expect(takeClaimCode()).toBeUndefined()
  })

  it('prefers the address to the storage', () => {
    window.sessionStorage.setItem('agency.claimCode', 'oldCode1234567890')
    goTo(`/claim/continue#code=${CODE}`)
    expect(takeClaimCode()).toBe(CODE)
  })

  it('is still there when the page asks again, though the address has lost it, until it is forgotten', () => {
    goTo(`/claim/continue#code=${CODE}`)
    expect(takeClaimCode()).toBe(CODE)
    expect(takeClaimCode()).toBe(CODE)
    forgetClaimCode()
    expect(takeClaimCode()).toBeUndefined()
  })

  it('is gone from this tab’s storage once forgotten', () => {
    window.sessionStorage.setItem('agency.claimCode', CODE)
    expect(takeClaimCode()).toBe(CODE)
    forgetClaimCode()
    expect(window.sessionStorage.getItem('agency.claimCode')).toBeNull()
  })

  it('is never written to the page’s own storage', () => {
    goTo(`/claim/continue#code=${CODE}`)
    takeClaimCode()
    expect(JSON.stringify({ ...window.localStorage })).not.toContain(CODE)
    expect(JSON.stringify({ ...window.sessionStorage })).not.toContain(CODE)
  })
})
