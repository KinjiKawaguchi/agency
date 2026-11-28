import sample from '../../../../testdata/client/org-members.json' with { type: 'json' }
import type { OrgMembers } from '../../api/clientApi.ts'
import { failure, json, type MockFeature } from './feature.ts'

/** The owner's list of an org's members, their devices and open seats (`GET /api/orgs/{org}/members`): the node's own sample for the org it describes. */
export interface MembersOptions {
  /** A node that serves the members to its owner and lists the `members` capability; otherwise the route answers 404. */
  members?: boolean
}

const PATH = /^\/api\/orgs\/([^/]+)\/members$/

/** The members of an org as the sample says them, or nothing for an org the sample does not describe. */
export const membersOf = (org: string): OrgMembers | undefined => (org === sample.org ? (sample as OrgMembers) : undefined)

export const membersFeature: MockFeature<MembersOptions, MembersOptions> = {
  capability: 'members',
  isOn: (options) => options.members === true,
  answer: (request, url) => {
    const matched = PATH.exec(url.pathname)
    if (matched === null || request.method !== 'GET') return undefined
    const members = membersOf(matched[1] ?? '')
    return members === undefined ? failure(404, 'No such organization.') : json(members)
  },
  isOwnerRoute: (pathname) => PATH.test(pathname),
  fromQuery: (query) => (query?.get('members') === '1' ? { members: true } : {}),
  toQuery: ({ members }) => (members === true ? [['members', '1']] : []),
}
