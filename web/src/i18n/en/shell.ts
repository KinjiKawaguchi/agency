// The frame of every screen: the app’s own messages, the menu, the settings, the person, the time, and what shows while a page loads.

export const app = {
  name: 'Agency',
  loading: 'Loading…',
  replica: {
    staleCopy: (when: string, reason: string) => `Showing a copy of this team from ${when}. Its node could not be reached: ${reason}`,
    neverRead: (reason: string) => `This team’s node has not been read yet: ${reason}`,
  },
  unreachable: (reason: string) => `Could not reach the node: ${reason}`,
  signInExpired: { message: 'Your sign-in expired. Reload to sign in again.', reload: 'Reload', nothingToShow: 'Nothing can be shown until you sign in again.' },
  pageLoad: {
    failed: 'This page could not be loaded.',
    tryAgain: 'Try again',
    newVersion: 'This page could not be loaded. A new version of Agency may have been installed, so what this tab has is out of date.',
    reload: 'Reload',
  },
} as const

export const nav = {
  label: 'Main menu',
  desk: 'Decisions',
  overview: 'Overview',
  people: 'Participants and work',
  org: 'Org chart',
  organization: 'Organization',
  results: 'Results',
  section: (group: string) => `${group} pages`,
  costs: 'Costs',
  evaluation: 'Evaluation',
  live: 'Live',
  outcomes: 'Outcomes',
  devices: 'Devices',
  members: 'Members',
  history: 'History',
} as const

export const settings = {
  theme: 'Theme',
  themeAuto: 'Auto',
  themeLight: 'Light',
  themeDark: 'Dark',
  language: 'Language',
} as const

export const person = {
  states: {
    working: 'Working',
    'needs-you': 'Waiting for you',
    waiting: 'Waiting for someone',
    idle: 'Idle',
    away: 'Away',
  },
} as const

export const time = {
  justNow: 'just now',
} as const

export const loading = {
  label: 'Loading…',
  failed: (what: string, reason: string) => `Could not load ${what}: ${reason}`,
  retryIn: (seconds: number) => `Trying again in ${seconds} s.`,
  retrying: 'Trying again…',
  tryAgain: 'Try again',
  what: { overview: 'the overview', orgChart: 'the org chart', people: 'participants and work' },
} as const
