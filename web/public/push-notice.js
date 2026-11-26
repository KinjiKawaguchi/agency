// What a push notification says and where it leads, as plain functions that use nothing of the worker. The service
// worker (sw.js) loads this file and only calls it, so that what decides is tested (src/pwa/pushNotice.test.ts).
// The payload is the node's notice, as the iPhone's is: `{ agency: { type?, org?, team?, request?, id? }, badge?, title?,
// body?, detail? }`. A request has `title` (the team's label) and `body` (its title); a device or a refused decision has a
// `detail` (the device's label, the decision's title). Text is there only when the node allows it (`hideTitles` leaves it
// out); without it the notification says only what kind it is, and nothing is added to it. A recovery of an org that was
// started (`type: "recovery"`, `org`, and `startedAt`, `finishesAt` when the node gives them) and a held root change (`type:
// "root"`, `org`, `id`, `waiting`) are worded here, in the viewer's language, and lead to the desk.
;(function (scope) {
  /** A plain identifier: letters, digits, `.`, `_`, `-`. Anything else is never used to make an address or a tag. */
  const PLAIN = /^[A-Za-z0-9._-]{1,200}$/
  const isPlain = (value) => typeof value === 'string' && PLAIN.test(value) && value !== '.' && value !== '..'

  const TITLE_CHARS = 120
  const BODY_CHARS = 240

  // The words of the iPhone's alerts (apple/Shared/*.lproj/Localizable.strings) and of the page's own notices, so that one event
  // reads the same on every surface; src/pwa/pushWorker.test.ts compares them with the iPhone's file.
  const WORDS = {
    en: {
      app: 'Agency',
      request: 'A request is waiting for you',
      device: (label) => (label === undefined ? 'A device is waiting to be added' : 'Device \u201c' + label + '\u201d is waiting to be added'),
      decision: (title) => (title === undefined ? 'A decision was refused' : 'The decision on \u201c' + title + '\u201d was refused'),
      recovery: (org, at) =>
        (org === undefined ? 'A recovery of an org was started' : 'A recovery of ' + org + ' was started') + (at.started ? ' at ' + at.started : '') + '. ' + (at.finishes ? 'It takes effect at ' + at.finishes + '. ' : '') + 'Open the desk to see it.',
      root: (org, n) => (n > 1 ? n + ' changes' : 'A change') + (org === undefined ? '' : ' to ' + org) + (n > 1 ? ' wait' : ' waits') + ' for the root key\u2019s signature. Open the desk to see ' + (n > 1 ? 'them' : 'it') + '.',
    },
    ja: {
      app: 'Agency',
      request: 'あなたを待っている依頼があります',
      device: (label) => (label === undefined ? '端末が追加を待っています' : '端末\u300c' + label + '\u300d\u304c追加を待っています'),
      decision: (title) => (title === undefined ? '決定が断られました' : '\u300c' + title + '\u300d\u306e決定は断られました'),
      recovery: (org, at) =>
        (org === undefined ? '組織のルート鍵の回復が始まりました' : org + ' のルート鍵の回復が始まりました') + (at.started ? '（' + at.started + '）' : '') + '。' + (at.finishes ? at.finishes + ' に有効になります。' : '') + '机で確認してください。',
      root: (org, n) => (org === undefined ? '' : org + ' の') + '変更' + (n > 1 ? ' ' + n + ' 件が' : 'が') + '、ルート鍵の署名を待っています。机で確認してください。',
    },
  }

  // A character that reorders or hides how a line reads becomes `?`, as the node and the page print it (src/shared/printable.ts).
  const HIDDEN = /[\u0000-\u001F\u007F-\u009F\u061C\u200B\u200E\u200F\u202A-\u202E\u2060\u2066-\u2069\uFEFF]/g
  // The mark comes first, then the trim: `trim` takes U+FEFF and the control whitespace off, and the page would have said a mark there.
  const cut = (value, length) => {
    const text = typeof value === 'string' ? value.replace(HIDDEN, '?').trim() : ''
    return text === '' ? undefined : text.slice(0, length)
  }

  /** A time as the viewer reads one (their locale and zone), or nothing when it is not a time. */
  function timeOf(value, language) {
    // Only a time of the node's own shape (RFC 3339): `Date.parse` takes many words and numbers for a date, such as '12' or 'Mar 3'.
    const ms = typeof value === 'string' && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:\d{2})$/.test(value) ? Date.parse(value) : NaN
    if (Number.isNaN(ms)) return undefined
    try {
      return new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }).format(new Date(ms))
    } catch {
      return undefined
    }
  }

  /** The payload as an object, or nothing when it is not one. */
  function parse(data) {
    let value = data
    if (typeof data === 'string') {
      try {
        value = JSON.parse(data)
      } catch {
        return undefined
      }
    }
    return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : undefined
  }

  /** `request` unless the payload says `device`, `decision`, `recovery` or `root`; a type it does not know gives nothing. */
  function kindOf(payload) {
    const agency = payload.agency
    if (typeof agency !== 'object' || agency === null) return undefined
    if (agency.type === undefined) return 'request'
    const known = ['device', 'decision', 'recovery', 'root']
    return known.includes(agency.type) ? agency.type : undefined
  }

  /**
   * Names what the notice is about, so that a browser replaces one notification with another about the same thing
   * (the page's own and this one, for one event): `request/<org>/<team>/<id>`, `decision/<id>`, `device`. The same
   * strings the page uses (src/notifications/present.ts), and `recovery/<org>`. No tag when an identifier is not a plain one.
   */
  function tagOf(payload) {
    const agency = payload.agency
    switch (kindOf(payload)) {
      case 'device':
        return 'device'
      case 'decision':
        return isPlain(agency.id) ? 'decision/' + agency.id : undefined
      case 'recovery':
        return isPlain(agency.org) ? 'recovery/' + agency.org : undefined
      case 'root':
        return isPlain(agency.id) ? 'root/' + agency.id : undefined
      case 'request':
        return isPlain(agency.org) && isPlain(agency.team) && isPlain(agency.request) ? 'request/' + agency.org + '/' + agency.team + '/' + agency.request : undefined
      default:
        return undefined
    }
  }

  /**
   * Where a tap goes: the request on the desk (`item` is `<org>:<team>:<request>`), the devices screen, or the desk. An
   * address is made only of plain identifiers; anything else, a kind that is not known, and a payload with no `agency`
   * lead to the desk.
   */
  function targetOf(payload) {
    const kind = kindOf(payload)
    if (kind === 'device') return '/devices'
    const agency = payload.agency
    if (kind === 'request' && isPlain(agency.org) && isPlain(agency.team) && isPlain(agency.request)) {
      return '/desk?item=' + encodeURIComponent(agency.org + ':' + agency.team + ':' + agency.request)
    }
    return '/desk'
  }

  /**
   * What to show: the node's title and body when it sent them, else the wording of the kind in the viewer's
   * language. A payload that is not one gives a plain "Agency" notification that opens the desk. Never `renotify`:
   * when a notification replaces another about the same thing, it does so quietly.
   */
  function contentOf(data, language) {
    const words = WORDS[typeof language === 'string' && language.toLowerCase().startsWith('ja') ? 'ja' : 'en']
    const payload = parse(data)
    const kind = payload === undefined ? undefined : kindOf(payload)
    if (payload === undefined || kind === undefined) return { title: words.app, body: '', tag: undefined, url: '/desk' }
    const tag = tagOf(payload)
    const url = targetOf(payload)
    if (kind === 'request') {
      const title = cut(payload.title, TITLE_CHARS)
      const body = cut(payload.body, BODY_CHARS)
      return title !== undefined && body !== undefined ? { title, body, tag, url } : { title: words.app, body: words.request, tag, url }
    }
    // A recovery and a held root change are worded here, in the viewer's language: the node's English names a time in its own way
    // and a label, and the desk (where a tap leads) says the rest. A recovery says its times in the viewer's locale and zone.
    const org = isPlain(payload.agency.org) ? payload.agency.org : undefined
    if (kind === 'recovery') {
      const at = { started: timeOf(payload.agency.startedAt, language), finishes: timeOf(payload.agency.finishesAt, language) }
      return { title: words.app, body: words.recovery(org, at), tag, url }
    }
    if (kind === 'root') return { title: words.app, body: words.root(org, payload.agency.waiting >= 2 && payload.agency.waiting <= 9999 && Number.isInteger(payload.agency.waiting) ? payload.agency.waiting : 1), tag, url }
    return { title: words.app, body: words[kind](cut(payload.detail, BODY_CHARS)), tag, url }
  }

  /** Whether to show it: not while a window of the app is in front, where the page itself tells what is new. */
  function shouldShow(windows) {
    return !windows.some((one) => one.focused === true && one.visibilityState !== 'hidden')
  }

  /** The window to bring forward on a tap: one of the app (same origin) if there is any, else a new one is opened. */
  function windowToFocus(windows) {
    return windows.find((one) => one.focused === true) || windows[0]
  }

  scope.AgencyNotice = { isPlain, parse, kindOf, tagOf, targetOf, contentOf, shouldShow, windowToFocus }
})(typeof self === 'undefined' ? globalThis : self)
