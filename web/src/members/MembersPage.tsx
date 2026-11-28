import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { meQuery, membersQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { OrgMembersView } from './OrgMembersView'
import { useMembersAccess } from './useMembersAccess'
import './members.css'

interface PageProps {
  /** The org to show; the first the owner can read when absent or unknown. */
  org: string | undefined
  onChange: (org: string) => void
}

/** What the node says of an org's people, for its owner: members and their devices, open seats, external parties, and what waits for the root key. */
export function MembersPage({ org, onChange }: PageProps) {
  const { t } = useI18n()
  const api = useApi()
  const access = useMembersAccess()
  const me = useQuery(meQuery(api))
  if (access.status === 'loading') return <Loading><Skeleton kind="short" /></Loading>
  if (access.status === 'unknown') return <p className="empty" role="alert">{failureText(t, access.error)}</p>
  if (access.status === 'none') return <p className="empty">{t.members.unavailable}</p>
  const chosen = access.orgs.find((one) => one === org) ?? access.orgs[0]
  const labels = new Map((me.data?.orgs ?? [] /* stand-in-ok(routing): names for the orgs of the picker; an org with no name here is listed by its id */).map((one) => [one.id, one.label]))
  return (
    <>
      <h1 className="page-title">{t.members.title}</h1>
      <p className="muted mb-lead">{t.members.lead}</p>
      {chosen === undefined ? null : (
        <>
          {access.orgs.length > 1 ? (
            <label className="mb-org">
              <span className="muted">{t.members.org}</span>
              <select value={chosen} onChange={(event) => onChange(event.target.value)}>
                {access.orgs.map((one) => (
                  <option key={one} value={one}>{labels.get(one) ?? one}</option>
                ))}
              </select>
            </label>
          ) : null}
          <OrgMembersFor org={chosen} />
        </>
      )}
    </>
  )
}

function OrgMembersFor({ org }: { org: string }) {
  const { t } = useI18n()
  const api = useApi()
  const members = useQuery(membersQuery(api, org))
  if (members.data === undefined) {
    return members.error === null ? <Loading quiet><Skeleton kind="short" /></Loading> : <p className="empty" role="alert">{failureText(t, members.error)}</p>
  }
  return (
    <>
      <OrgMembersView members={members.data} />
      <p className="muted mb-source">{t.members.source(`/api/orgs/${encodeURIComponent(org)}/members`)}</p>
    </>
  )
}
