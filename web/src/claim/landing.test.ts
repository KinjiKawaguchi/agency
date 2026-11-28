import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { forgetClaimCode, takeClaimCode } from './claimCode'

// The public landing page's script is a plain file of `public/`, like the worker; it is run here in the page of the test.
const SOURCE = readFileSync(join(process.cwd(), 'public', 'landing', 'claim.js'), 'utf8')
const HTML = readFileSync(join(process.cwd(), 'public', 'claim.html'), 'utf8')
const CODE = 'k3JfA9xQ2mVb7LzP'

interface Landing {
  codeOf: (hash: string) => string | undefined
  start: (win: Window, doc: Document) => boolean
}

function load(address: string, language = 'en-US'): Landing {
  window.history.replaceState(null, '', address)
  Object.defineProperty(window.navigator, 'language', { value: language, configurable: true })
  document.body.innerHTML = HTML.slice(HTML.indexOf('<main>'), HTML.indexOf('</main>') + 7)
  delete (window as { AgencyClaimLanding?: unknown }).AgencyClaimLanding
  new Function('self', SOURCE)(window)
  return (window as unknown as { AgencyClaimLanding: Landing }).AgencyClaimLanding
}

describe('the public landing page of a claim link', () => {
  beforeEach(() => {
    window.sessionStorage.clear()
    forgetClaimCode()
  })

  it('takes the code out of the address at once, keeps it for this tab, and leaves the person one way on', () => {
    load(`/claim#code=${CODE}`)
    expect(window.location.hash).toBe('')
    expect(window.location.pathname).toBe('/claim')
    expect(window.sessionStorage.getItem('agency.claimCode')).toBe(CODE)
    const go = document.getElementById('go') as HTMLAnchorElement
    expect(go.getAttribute('href')).toBe('/claim/continue')
    expect(go.hidden).toBe(false)
    expect(go.textContent).toBe('Sign in to continue')
    expect(document.body.textContent).not.toContain(CODE)
  })

  it('is read by the page the sign-in leads to, from this tab’s storage', () => {
    load(`/claim#code=${CODE}`)
    window.history.replaceState(null, '', '/claim/continue')
    expect(takeClaimCode()).toBe(CODE)
  })

  it('says it needs the link, and offers no sign-in, when there is no code or it is not one', () => {
    for (const address of ['/claim', '/claim#code=short', '/claim#code=<b>', '/claim#other=1']) {
      load(address)
      expect(window.sessionStorage.getItem('agency.claimCode'), address).toBeNull()
      expect((document.getElementById('go') as HTMLElement).hidden, address).toBe(true)
      expect(document.getElementById('lead')?.textContent, address).toBe('This page needs the link you were sent. Open that link again.')
      expect(window.location.hash, address).toBe('')
    }
  })

  it('is in Japanese for a browser that asks for it, and in English otherwise', () => {
    load(`/claim#code=${CODE}`, 'ja-JP')
    expect(document.getElementById('title')?.textContent).toBe('ポジションを受け取る')
    expect(document.documentElement.lang).toBe('ja')
    load(`/claim#code=${CODE}`, 'fr-FR')
    expect(document.getElementById('title')?.textContent).toBe('Take your seat')
  })

  it('goes on with the code in the address, as the link had it, only where the tab’s storage is blocked', () => {
    const blocked = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    try {
      load(`/claim#code=${CODE}`)
      expect((document.getElementById('go') as HTMLAnchorElement).getAttribute('href')).toBe(`/claim/continue#code=${CODE}`)
      expect(window.location.hash).toBe('')
    } finally {
      blocked.mockRestore()
    }
  })

  it('has a style that lets `hidden` hide the button, whatever display the button has', () => {
    const style = readFileSync(join(process.cwd(), 'public', 'landing', 'claim.css'), 'utf8')
    expect(style).toMatch(/\[hidden\]\s*\{\s*display:\s*none\s*!important/)
  })

  it('names nothing of the node or an organization, and reads nothing from the network', () => {
    expect(HTML).not.toMatch(/agency-token|<script>[^<]|fetch\(|XMLHttpRequest/)
    expect(SOURCE).not.toMatch(/fetch\(|XMLHttpRequest|localStorage|sendBeacon|console\./)
    expect(HTML).toContain('<link rel="stylesheet" href="/landing/claim.css"')
    expect(HTML).toContain('<script src="/landing/claim.js"></script>')
  })
})
