// The change history screen.

export const history = {
  title: 'History',
  lead: 'Every version of the org the node keeps, oldest first: when the root signed it, what its signature showed, and what it changed. It is read-only, and the signature is the node’s verdict, not this page’s.',
  org: 'Organization',
  noOrgs: 'You are in no organization.',
  loading: 'Asking the node…',
  unavailable: 'This node does not report the history yet.',
  empty: 'The node keeps no version of this org.',
  version: (number: number) => `Version ${number}`,
  signed: (when: string) => `signed ${when}`,
  signedBy: (key: string) => `by ${key}`,
  signatures: { valid: 'The signature is valid', invalid: 'The signature is not valid: the version was changed after it was signed, or was never signed by the org’s root', notChecked: 'The signature was not checked: the version before it is not kept here' },
  notKept: 'This node does not keep this version.',
  fork: 'It does not follow the version before it: the history forks here.',
  comparedWith: (number: number) => `Its changes are counted from version ${number}, since the versions between are not kept here.`,
  noChanges: 'No change is listed.',
  more: 'Show later versions',
  loadingMore: 'Asking the node…',
  moreFailed: (reason: string) => `Could not read later versions: ${reason}`,
  retry: 'Try again',
  source: (path: string) => `The same entries an agent reads: GET ${path}.`,
} as const
