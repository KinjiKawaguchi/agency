import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { historyQuery, meQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { VersionRow } from './VersionRow'
import { useHistoryAccess } from './useHistoryAccess'
import './history.css'

interface PageProps {
  /** The org to show; the first the owner can read when absent or unknown. */
  org: string | undefined
  onChange: (org: string) => void
}

/** What the node says of an org's history, for its owner: each version it keeps, oldest first, with who signed it, when, and what it changed. */
export function HistoryPage({ org, onChange }: PageProps) {
  const { t } = useI18n()
  const api = useApi()
  const access = useHistoryAccess()
  const me = useQuery(meQuery(api))
  if (access.status === 'loading') return <Loading><Skeleton kind="short" /></Loading>
  if (access.status === 'unknown') return <p className="empty" role="alert">{failureText(t, access.error)}</p>
  if (access.status === 'none') return <p className="empty">{t.history.unavailable}</p>
  const chosen = access.orgs.find((one) => one === org) ?? access.orgs[0]
  const labels = new Map((me.data?.orgs ?? [] /* stand-in-ok(routing): names for the orgs of the picker; an org with no name here is listed by its id */).map((one) => [one.id, one.label]))
  return (
    <>
      <h1 className="page-title">{t.history.title}</h1>
      <p className="muted hs-lead">{t.history.lead}</p>
      {chosen === undefined ? null : (
        <>
          {access.orgs.length > 1 ? (
            <label className="hs-org">
              <span className="muted">{t.history.org}</span>
              <select value={chosen} onChange={(event) => onChange(event.target.value)}>
                {access.orgs.map((one) => (
                  <option key={one} value={one}>{labels.get(one) ?? one}</option>
                ))}
              </select>
            </label>
          ) : null}
          <OrgHistory key={chosen} org={chosen} />
        </>
      )}
    </>
  )
}

function OrgHistory({ org }: { org: string }) {
  const { t } = useI18n()
  const api = useApi()
  const history = useInfiniteQuery(historyQuery(api, org))
  if (history.data === undefined) {
    return history.error === null ? <Loading quiet><Skeleton kind="short" /></Loading> : <p className="empty" role="alert">{failureText(t, history.error)}</p>
  }
  const entries = history.data.pages.flatMap((page) => page.entries)
  return (
    <>
      {entries.length === 0 ? (
        <p className="muted">{t.history.empty}</p>
      ) : (
        <ol className="hs-list">
          {entries.map((entry) => (
            <VersionRow key={entry.version} entry={entry} />
          ))}
        </ol>
      )}
      {history.hasNextPage ? (
        <div className="hs-more">
          {history.isFetchNextPageError && history.error !== null ? (
            <p className="empty" role="alert">{t.history.moreFailed(history.error.message)}</p>
          ) : null}
          <button className="btn small" type="button" disabled={history.isFetchingNextPage} onClick={() => void history.fetchNextPage()}>
            {history.isFetchingNextPage ? t.history.loadingMore : history.isFetchNextPageError ? t.history.retry : t.history.more}
          </button>
        </div>
      ) : null}
      <p className="muted hs-source">{t.history.source(`/api/orgs/${encodeURIComponent(org)}/history`)}</p>
    </>
  )
}
