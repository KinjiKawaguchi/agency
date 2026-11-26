const KEY = 'agency.claimCode'

/** What a code looks like: letters, digits, `-` and `_`, a length that cannot be guessed. Anything else is not one. */
const SHAPE = /^[A-Za-z0-9_-]{8,200}$/

/** The code of a link's fragment (`#code=…`), or nothing when there is none or it is not shaped like one. */
export function codeFromHash(hash: string): string | undefined {
  const code = new URLSearchParams(hash.replace(/^#/, '')).get('code')
  return code !== null && SHAPE.test(code) ? code : undefined
}

const session = (): Pick<Storage, 'getItem' | 'removeItem'> | undefined => {
  try {
    return window.sessionStorage
  } catch {
    return undefined
  }
}

/** The code, kept in memory from the moment it is taken, so that a page drawn twice (or tried again) does not lose it. */
let held: string | undefined

/**
 * Takes the claim link's code: from the fragment of the address, which is removed from the address at once (it
 * would otherwise stay in the history and in anything that copies the address), else from this tab's session storage,
 * where the public landing page left it before the sign-in. It is never written anywhere by this page.
 */
export function takeClaimCode(win: Pick<Window, 'location' | 'history'> = window): string | undefined {
  if (win.location.hash !== '') {
    const fromAddress = codeFromHash(win.location.hash)
    win.history.replaceState(win.history.state, '', win.location.pathname + win.location.search)
    if (fromAddress !== undefined) held = fromAddress
  }
  if (held !== undefined) return held
  const stored = session()?.getItem(KEY) ?? undefined
  if (stored !== undefined && SHAPE.test(stored)) held = stored
  return held
}

/** The code is spent, or cannot be used: it is gone from memory and from this tab. */
export function forgetClaimCode(): void {
  held = undefined
  try {
    session()?.removeItem(KEY)
  } catch {
    // Storage can be blocked; there is then nothing to remove.
  }
}
