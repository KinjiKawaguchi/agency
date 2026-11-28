// The notices the app gives of what is new (requests, participants, blocked work, devices, decisions), and where they are sent.

export const notices = {
  request: (count: number, title: string, team: string) => (count === 1 ? `A request waits for you: ${title} (${team}).` : `${count} requests now wait for you.`),
  participant: (count: number, label: string) => (count === 1 ? `${label} is waiting for you.` : `${count} participants are waiting for you.`),
  blocked: (count: number, title: string, team: string, reason: string | undefined) => (count === 1 ? `Blocked: ${title} (${team})${reason === undefined ? '' : `: ${reason}`}.` : `${count} work items are newly blocked.`),
  aDevice: 'A device',
  device: (count: number, label: string) => (count === 1 ? `${label} asks to be added as a device.` : `${count} devices ask to be added.`),
  decision: (state: string, count: number, title: string, reason: string | undefined) => {
    const named = count === 1 && title !== '' ? ` “${title}”` : ''
    if (state === 'refused') return count === 1 ? `A decision${named} was refused${reason === undefined ? '' : `: ${reason}`}` : `${count} decisions were refused.`
    if (state === 'queued') return count === 1 ? `A decision${named} is saved, not delivered: its node cannot be reached yet.` : `${count} decisions are saved, not delivered.`
    return count === 1 ? `A decision${named} was ${state}.` : `${count} decisions were ${state}.`
  },
  more: (count: number) => `${count} more things are new.`,
  // What a notice says outside the page when no title, label, or name may leave it: the iPhone's words for a push whose titles are hidden.
  hidden: {
    request: 'A request is waiting for you',
    participant: 'A participant is waiting for you',
    blocked: 'Work is blocked',
    device: 'A device is waiting to be added',
    decisionRefused: 'A decision was refused',
  },
  devicesMark: (count: number) => `${count} waiting to be approved`,
  destinations: {
    title: 'Where notices are sent',
    lead: 'Every browser and app this node sends notices to. When a device is removed, the notices to it stop by themselves; a registration that names no device stays until you stop it here, or until notices are turned off in that browser or app.',
    thisBrowser: 'this browser',
    turnOffHere: 'Turn off notices in this browser',
    loading: 'Asking the node…',
    failed: (reason: string) => `Could not read where notices are sent: ${reason}`,
    none: 'This node sends notices nowhere.',
    kinds: { browser: 'Browser', app: 'iPhone or Mac app' },
    registered: (when: string) => `registered ${when}`,
    tiedTo: (label: string) => `Tied to ${label}.`,
    tiedToNone: 'Tied to no device. It stays until you stop it.',
    tiedToGone: (id: string) => `Tied to a device that is not yours any more (${id}).`,
    stopFor: (label: string) => `Stop notices to ${label}`,
    stop: 'Stop notices',
    confirmTitle: (label: string) => `Stop notices to ${label}?`,
    confirmBody: (label: string, when: string) => `${label} (registered ${when}) will no longer get notices from this node. It registers itself again the next time that browser or app opens Agency with notices on. To stop them for good, turn notices off there, or remove its device on this screen.`,
    confirm: 'Stop notices',
    cancel: 'Cancel',
    stopping: 'Stopping…',
    stopped: (label: string) => `${label} no longer gets notices.`,
    stopFailed: (reason: string) => `Could not stop the notices: ${reason}`,
  },
  os: {
    title: 'Notifications in this browser',
    lead: 'Show what is new as a notification of the operating system while this page is hidden. It says what the page says, and stays in this browser.',
    turnOn: 'Turn on notifications',
    turnOff: 'Turn off notifications',
    isOn: 'On. A notification appears when something is new while this page is hidden. The page keeps reading in the background to see it, as far as the browser lets it, and not while the browser is closed.',
    isBlocked: 'This browser does not allow notifications for this page. Allow them in its site settings, then turn them on here.',
    isUnsupported: 'This browser cannot show notifications from a page.',
    push: {
      title: 'When the app is closed',
      lead: 'The node sends what is new to this browser through its push service, even when no page of it is open. The words are the node’s: a node that hides titles sends none.',
      turnOn: 'Turn on for a closed app',
      turnOff: 'Turn off for a closed app',
      isOn: 'On. The node tells this browser when something is new, with the page closed.',
      isFailing: (when: string, reason: string) => `The push service of this browser did not take the last notice (${when}): ${reason}. The node tries again for up to a day. If it keeps failing, turn this off and on again.`,
      lastTaken: (when: string) => `The last notice it took was at ${when}.`,
      neverTaken: 'It has taken none yet.',
      isStale: 'This browser was subscribed with a key the node no longer has, so it is not told. Subscribe again.',
      subscribeAgain: 'Subscribe again',
      failed: (message: string) => `Could not change it: ${message}`,
    },
  },
} as const
