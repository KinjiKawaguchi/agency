import { useQuery } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useApi } from '../api/context'
import { orgDefinitionQuery, useTeams } from '../api/queries'
import { useI18n } from '../i18n'
import { CenteredFrame } from './CenteredFrame'
import { OrgTree } from './OrgTree'
import { buildOrgTree } from './tree'
import './org.css'

interface OrgChartPageProps {
  /** The organization to draw; the first one the viewer belongs to when absent. */
  org: string | undefined
  /** The department to focus on; the whole organization when absent. */
  unit: string | undefined
}

export function OrgChartPage({ org, unit }: OrgChartPageProps) {
  const { t } = useI18n()
  const api = useApi()
  const navigate = useNavigate()
  const { me, teams, error } = useTeams()
  const selected = me?.orgs.find((one) => one.id === org) ?? me?.orgs[0]
  const definition = useQuery({ ...orgDefinitionQuery(api, selected?.id ?? ''), enabled: selected !== undefined })

  if (me === undefined || selected === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : t.app.unreachable(error.message)}</p>
  }
  if (definition.data === undefined) {
    return <p className={definition.error === null ? 'muted' : 'empty'} role={definition.error === null ? undefined : 'alert'}>{definition.error === null ? t.app.loading : t.app.unreachable(definition.error.message)}</p>
  }

  const tree = buildOrgTree(definition.data, teams, me.account.id, unit)
  return (
    <>
      <div className="page-head">
        <h1 className="page-title">{t.org.title(selected.label)}</h1>
        {me.orgs.length > 1 ? (
          <select aria-label={t.org.chooseOrg} value={selected.id} onChange={(event) => void navigate({ to: '/org', search: { org: event.target.value } })}>
            {me.orgs.map((one) => (
              <option key={one.id} value={one.id}>{one.label}</option>
            ))}
          </select>
        ) : null}
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
