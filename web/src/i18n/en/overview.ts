// The overview screen.

export const overview = {
  title: 'Overview',
  lead: (requests: number, teams: number) =>
    requests === 0 ? 'Nothing waits for you in any team.' : `${requests} ${requests === 1 ? 'request waits' : 'requests wait'} for you, in ${teams} ${teams === 1 ? 'team' : 'teams'}.`,
  leadPartial: (requests: number, teams: number, unread: number) =>
    `${requests === 0 ? 'Nothing waits for you in the teams that answered.' : `${requests} ${requests === 1 ? 'request waits' : 'requests wait'} for you, in ${teams} ${teams === 1 ? 'team' : 'teams'} that answered.`} ${unread} ${unread === 1 ? 'team' : 'teams'} could not be read.`,
  noTeams: 'This org has no teams yet.',
  waitingForYou: (count: number) => (count === 1 ? '1 request waits for you' : `${count} requests wait for you`),
  nothingWaits: 'Nothing waits for you.',
  working: (working: number, total: number) => `${working} of ${total} participants working`,
  noParticipants: 'No participants reported.',
  tags: { mine: 'Your turn', stuck: 'Stuck', idle: 'Idle', failed: 'Not loaded' },
  blocked: (count: number) => (count === 1 ? '1 blocked item' : `${count} blocked items`),
  nobodyWorking: 'Nobody is working.',
  usage: (amount: string) => `${amount} this week, API-priced`,
  usageNone: 'Token use is not reported.',
  usageFailed: 'Token use could not be read.',
  dataFrom: (relative: string) => `Data from ${relative}`,
  staleFrom: (relative: string, reason: string) => `Data from ${relative}. The team’s node could not be reached: ${reason}`,
  neverRead: (reason: string) => `Not read yet: ${reason}`,
  loadFailed: (reason: string) => `Could not load this team: ${reason}`,
  funding: 'Usage limits',
} as const
