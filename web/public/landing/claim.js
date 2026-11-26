// The script of the public landing page of a claim link (/claim). It runs before any sign-in, so it knows nothing of the node:
// it takes the code from the address (`#code=…`), removes the fragment from the address at once, keeps the code in this
// tab's session storage for the page that comes after the sign-in (/claim/continue), and words the page in the browser's
// language. The code goes nowhere else: no request, no log, no other storage.
;(function (scope) {
  const KEY = 'agency.claimCode'
  const SHAPE = /^[A-Za-z0-9_-]{8,200}$/

  const WORDS = {
    en: {
      title: 'Take your seat',
      lead: 'You were invited to take a seat.',
      go: 'Sign in to continue',
      none: 'This page needs the link you were sent. Open that link again.',
    },
    ja: {
      title: '席を受け取る',
      lead: '席を受け取るよう招かれています。',
      go: 'サインインして続ける',
      none: 'このページには、送られたリンクが必要です。そのリンクを開き直してください。',
    },
  }

  const wordsFor = (language) => WORDS[typeof language === 'string' && language.toLowerCase().startsWith('ja') ? 'ja' : 'en']

  /** The code of a fragment, or nothing when there is none or it is not shaped like one. */
  function codeOf(hash) {
    const code = new URLSearchParams(String(hash).replace(/^#/, '')).get('code')
    return code !== null && SHAPE.test(code) ? code : undefined
  }

  /** Runs the page: `win` and `doc` are the browser's, or a stand-in in a test. Answers whether there is a code to go on with. */
  function start(win, doc) {
    const code = codeOf(win.location.hash)
    let isKept = false
    if (win.location.hash !== '') win.history.replaceState(win.history.state, '', win.location.pathname + win.location.search)
    if (code !== undefined) {
      try {
        win.sessionStorage.setItem(KEY, code)
        isKept = true
      } catch {
        isKept = false
      }
    }
    const words = wordsFor(win.navigator && win.navigator.language)
    doc.documentElement.lang = words === WORDS.ja ? 'ja' : 'en'
    doc.getElementById('title').textContent = words.title
    const go = doc.getElementById('go')
    // Without a code, there is nothing to sign in for: say so, and offer nothing.
    if (code === undefined) {
      doc.getElementById('lead').textContent = words.none
      go.hidden = true
      return false
    }
    doc.getElementById('lead').textContent = words.lead
    go.textContent = words.go
    // Where the storage of this tab is blocked, the code has no place to wait for the sign-in: it goes on in the address, as the
    // link had it, which is all that can be done (the fragment may not survive the sign-in, and then the owner makes a new link).
    if (!isKept) go.setAttribute('href', '/claim/continue#code=' + code)
    return true
  }

  scope.AgencyClaimLanding = { codeOf, wordsFor, start, KEY }
  if (typeof document !== 'undefined' && document.getElementById('go') !== null) start(scope, document)
})(typeof self === 'undefined' ? globalThis : self)
