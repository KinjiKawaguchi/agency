import { useQueries, useQuery } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useApi } from '../api/context'
import { orgDefinitionQuery, useTeams } from '../api/queries'
import { failureText } from '../api/failure'
import { useI18n } from '../i18n'
import { CenteredFrame } from './CenteredFrame'
import { AllOrgsTree, OrgTree } from './OrgTree'
import { buildAllOrgsTree, buildOrgTree } from './tree'
import './org.css'

/** The value of the organization menu that draws every org in one chart. */
export const ALL_ORGS = 'all'

interface OrgChartPageProps {
  /** The organization to draw, or `all` for every one; every one when the viewer belongs to several and none is named, else their only one. */
  org: string | undefined
  /** The department to focus on; the whole organization when absent. */
  unit: string | undefined
}

/** Every org of the account in one chart, under the account. */
function AllOrgsChart({ onChoose }: { onChoose: (org: string) => void }) {
  const { t } = useI18n()
  const api = useApi()
  const { me, teams, error } = useTeams()
  const definitions = useQueries({ queries: (me?.orgs ?? []).map((org) => orgDefinitionQuery(api, org.id)) })
  if (me === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : failureText(t, error)}</p>
  }
  const inputs = me.orgs.map((org, index) => ({ org, definition: definitions[index]?.data, failed: definitions[index]?.error != null }))
  const orgs = buildAllOrgsTree(inputs, teams, me.account.id)
  return (
    <>
      <div className="page-head">
        <h1 className="page-title">{t.org.allTitle}</h1>
        <OrgMenu value={ALL_ORGS} orgs={me.orgs} onChoose={onChoose} />
      </div>
      <CenteredFrame resetKey={ALL_ORGS}>
        <AllOrgsTree account={me.account.label} orgs={orgs} />
      </CenteredFrame>
    </>
  )
}

/** The menu of the chart's organizations, with "all" first when there is more than one. */
function OrgMenu({ value, orgs, onChoose }: { value: string; orgs: ReadonlyArray<{ id: string; label: string }>; onChoose: (org: string) => void }) {
  const { t } = useI18n()
  if (orgs.length < 2) return null
  return (
    <select aria-label={t.org.chooseOrg} value={value} onChange={(event) => onChoose(event.target.value)}>
      <option value={ALL_ORGS}>{t.org.allOrgs}</option>
      {orgs.map((one) => (
        <option key={one.id} value={one.id}>{one.label}</option>
      ))}
    </select>
  )
}

export function OrgChartPage({ org, unit }: OrgChartPageProps) {
  const { t } = useI18n()
  const api = useApi()
  const navigate = useNavigate()
  const { me, teams, error } = useTeams()
  const isAll = me !== undefined && me.orgs.length > 1 && (org === undefined || org === ALL_ORGS)
  const selected = me?.orgs.find((one) => one.id === org) ?? me?.orgs[0]
  const definition = useQuery({ ...orgDefinitionQuery(api, selected?.id ?? ''), enabled: selected !== undefined && !isAll })
  if (isAll) return <AllOrgsChart onChoose={(next) => void navigate({ to: '/org', search: { org: next } })} />

  if (me === undefined || selected === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : failureText(t, error)}</p>
  }
  if (definition.data === undefined) {
    return <p className={definition.error === null ? 'muted' : 'empty'} role={definition.error === null ? undefined : 'alert'}>{definition.error === null ? t.app.loading : failureText(t, definition.error)}</p>
  }

  const tree = buildOrgTree(definition.data, teams, me.account.id, unit)
  return (
    <>
      <div className="page-head">
        <h1 className="page-title">{t.org.title(selected.label)}</h1>
        <OrgMenu value={selected.id} orgs={me.orgs} onChoose={(next) => void navigate({ to: '/org', search: { org: next } })} />
      </div>
      {tree.focus === undefined ? null : (
        <p className="oc-up">
          <Link to="/org" search={{ org: selected.id }}>{t.org.whole}</Link>
        </p>
      )}
      {tree.children.length === 0 && tree.focus !== undefined ? (
        <p className="empty">{t.org.emptyDepartment}</p>
      ) : (
        <CenteredFrame resetKey={`${selected.id}/${unit ?? ''}`}>
          <OrgTree tree={tree} org={selected.id} orgLabel={selected.label} />
        </CenteredFrame>
      )}
    </>
  )
}
