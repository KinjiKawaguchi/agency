// Every user-visible string lives in this catalog. Other locales implement the same `Messages` type,
// so a missing or mistyped key is a compile error.

/** Text that surrounds an inline link: [before, after]. */
export type Around = readonly [before: string, after: string]

export const en = {
  app: {
    name: 'Agency',
    loading: 'Loading…',
    unreachable: (reason: string) => `Could not reach the node: ${reason}`,
  },
  nav: {
    label: 'Main menu',
    desk: 'Decisions',
    people: 'People and work',
    org: 'Org chart',
    costs: 'Costs',
    comingSoon: 'This screen is not ready yet.',
  },
  settings: {
    theme: 'Theme',
    themeAuto: 'Auto',
    themeLight: 'Light',
    themeDark: 'Dark',
    language: 'Language',
  },
  person: {
    states: {
      working: 'Working',
      'needs-you': 'Waiting for you',
      waiting: 'Waiting for someone',
      idle: 'Idle',
      away: 'Away',
    },
  },
  time: {
    justNow: 'just now',
  },
  desk: {
    briefing: {
      greeting: (name: string, when: string) => `For ${name} · ${when}`,
      someoneWaiting: (count: number) => (count === 1 ? '1 person is waiting for your answer.' : `${count} people are waiting for your answer.`),
      itemsOnDesk: (count: number) => (count === 1 ? 'There is 1 item on your desk.' : `There are ${count} items on your desk.`),
      nothingToDecide: 'Nothing needs your decision right now.',
      nextProposal: ['Start by approving ', '.'] satisfies Around,
      nextQuestion: (drafter: string): Around => [`Start by answering ${drafter}’s question `, '.'],
      nextPending: ['Start with ', '.'] satisfies Around,
      peopleWorking: (working: number, total: number) => `${working} of ${total} people are working.`,
      allOnDesk: (count: number) => `${count} items on your desk in all.`,
      seePeople: 'See people and work',
    },
    pile: {
      label: 'Documents waiting',
      empty: 'No documents have arrived.',
      groups: {
        proposal: 'Proposals for you to approve',
        question: 'Questions for you',
        pendingDecision: 'Decisions waiting on the board',
        others: 'Waiting on someone else',
      },
      kinds: { proposal: 'Proposal', question: 'Question', pendingDecision: 'Pending decision' },
      blocking: (count: number) => (count === 1 ? '1 person waiting' : `${count} people waiting`),
      showMore: (count: number) => `Show ${count} more`,
    },
    paper: {
      blank: 'Pick a document on the left to open it here.',
      types: { proposal: 'Proposal for approval', question: 'Question', pendingDecision: 'Request for a decision' },
      team: (org: string, team: string) => `${org} / ${team}`,
      stoppedBy: (names: string) => `${names} cannot continue until this is decided`,
      stamps: {
        drafted: 'Drafted',
        approved: 'Approved',
        answered: 'Answered',
        label: (drafter: string, step: string, decider: string) => `Drafted by ${drafter}, ${step} by ${decider}`,
      },
      waitingOn: (name: string) => `Waiting for ${name} to decide. You can read it here but not answer it.`,
      recommended: 'Drafter’s pick',
      options: 'Options',
      notePlaceholderWithOptions: 'Add conditions or notes (optional)',
      notePlaceholderOpen: 'Write your answer',
      noteLabel: 'Notes',
      verbs: { proposal: 'Approve', question: 'Answer', pendingDecision: 'Send decision' },
      verbsDone: { proposal: 'approved', question: 'answered', pendingDecision: 'sent a decision on' },
      send: (verb: string) => verb,
      sendWith: (verb: string, choice: string) => `${verb} with “${choice}”`,
      sending: 'Sending…',
      sealedNote: (name: string, done: string) => `${name} ${done} this. It is on its way to the drafter.`,
      readDocument: (characters: string) => `Read the document (about ${characters} characters)`,
      shortcutSend: 'to send',
      shortcutPick: 'number keys pick an option',
      needAnswer: 'Pick an option or write an answer first.',
      needDecision: 'Write your decision first.',
      sent: (drafter: string) => `Sent. ${drafter} will get it.`,
      sendFailed: (reason: string) => `Could not send: ${reason}`,
      thread: (name: string, choice: string | null) => (choice === null ? name : `${name} chose “${choice}”`),
    },
  },
} as const

type Widen<T> = T extends string
  ? string
  : T extends readonly [string, string]
    ? Around
    : T extends (...args: infer A) => infer R
      ? (...args: A) => Widen<R>
      : T extends object
        ? { -readonly [K in keyof T]: Widen<T[K]> }
        : T

export type Messages = Widen<typeof en>
