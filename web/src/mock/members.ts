// The mock node's `GET /api/orgs/{org}/members`: the node's own sample (`testdata/client/org-members.json`) for the org it describes, and 404 for any
// other, as for an org the owner is not in. A node that lists the `members` capability serves it; the owner's alone, as the node does.
import sample from '../../../testdata/client/org-members.json' with { type: 'json' }
import type { OrgMembers } from '../api/clientApi.ts'

/** The members of an org as the sample says them, or nothing for an org the sample does not describe. */
export const membersOf = (org: string): OrgMembers | undefined => (org === sample.org ? (sample as OrgMembers) : undefined)
