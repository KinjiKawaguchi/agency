/**
 * Behind Cloudflare Access, an expired session turns every call into a redirect to the Access login on
 * another origin. The browser fails such a call as a network error, with no HTTP answer to read, so the
 * client has to recognize the case from the page's own address.
 */
export class SignInExpiredError extends Error {
  constructor() {
    super('The sign-in expired.')
    this.name = 'SignInExpiredError'
  }
}

const LOOPBACK_HOSTS: ReadonlySet<string> = new Set(['localhost', '127.0.0.1', '[::1]', '::1'])

export const isLoopbackHost = (hostname: string): boolean => LOOPBACK_HOSTS.has(hostname) || hostname.endsWith('.localhost')

/**
 * A call that failed without any HTTP answer, from a page that was not loaded from this machine, is most
 * likely a redirect to the sign-in. A browser that knows it is offline is not: that is just a lost connection.
 */
export function looksLikeExpiredSignIn(page: { hostname: string }, isOnline: boolean): boolean {
  return isOnline && !isLoopbackHost(page.hostname)
}
