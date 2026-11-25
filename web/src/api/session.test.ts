import { isLoopbackHost, looksLikeExpiredSignIn } from './session'

describe('looksLikeExpiredSignIn', () => {
  it('takes a call that failed without an answer, from a page away from this machine, for an expired sign-in', () => {
    expect(looksLikeExpiredSignIn({ hostname: 'agency.example.test' }, true)).toBe(true)
  })

  it.each(['localhost', '127.0.0.1', '[::1]'])('does not, from a page on this machine (%s), where there is no sign-in to expire', (hostname) => {
    expect(isLoopbackHost(hostname)).toBe(true)
    expect(looksLikeExpiredSignIn({ hostname }, true)).toBe(false)
  })

  it('does not when the browser knows it is offline: that is a lost connection', () => {
    expect(looksLikeExpiredSignIn({ hostname: 'agency.example.test' }, false)).toBe(false)
  })

  it('does not take a host that only looks like loopback for one', () => {
    expect(isLoopbackHost('localhost.example.test')).toBe(false)
    expect(isLoopbackHost('127.0.0.1.example.test')).toBe(false)
    // The node admits only 127.0.0.1 and localhost as its own address, so a name under .localhost is a named host.
    expect(isLoopbackHost('agency.localhost')).toBe(false)
  })
})
