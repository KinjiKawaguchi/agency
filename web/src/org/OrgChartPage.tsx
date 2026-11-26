import { useQueries, useSuspenseQuery } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { useDeferredValue } from 'react'
import { useApi } from '../api/context'
import type { Me } from '../api/clientApi'
import { orgDefinitionQuery, useSuspenseTeamStates } from '../api/queries'
import type { Team } from '../api/teams'
import { useI18n } from '../i18n'
import { Region } from '../shared/Region'
import { OrgChartSkeleton } from './OrgChartSkeleton'
import { OrgChartViewer } from './OrgChartViewer'
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

interface ChartProps {
  me: Me
  teams: readonly Team[]
  /** The `org/team` keys of the teams still being read. */
  loading: ReadonlySet<string>
  onChoose: (org: string) => void
}

/** Every org of the account in one chart, under the account. An org whose definition is still being read, or cannot be, is a box that says so; the others are drawn. */
function AllOrgsChart({ me, teams, loading, onChoose }: ChartProps) {
  const { t } = useI18n()
  const api = useApi()
  const definitions = useQueries({ queries: me.orgs.map((org) => orgDefinitionQuery(api, org.id)) })
  const inputs = me.orgs.map((org, index) => ({ org, definition: definitions[index]?.data, failed: definitions[index]?.error != null }))
  const orgs = buildAllOrgsTree(inputs, teams, me.account.id, loading)
  return (
    <>
      <div className="page-head">
        <h1 className="page-title">{t.org.allTitle}</h1>
        <OrgMenu value={ALL_ORGS} orgs={me.orgs} onChoose={onChoose} />
      </div>
      <OrgChartViewer resetKey={`${ALL_ORGS}/${orgs.filter((one) => one.state === 'ready').length}`} fileName="org-chart-all-orgs">
        <AllOrgsTree account={me.account.label} orgs={orgs} />
      </OrgChartViewer>
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

/** One org's chart, optionally focused on a department. Its definition suspends the region; failing for good, it is the region's failure. */
function OneOrgChart({ me, teams, loading, org, unit, onChoose }: ChartProps & { org: Me['orgs'][number]; unit: string | undefined }) {
  const { t } = useI18n()
  const api = useApi()
  const { data: definition } = useSuspenseQuery(orgDefinitionQuery(api, org.id))
  const tree = buildOrgTree(definition, teams, me.account.id, unit, loading)
  return (
    <>
      <div className="page-head">
        <h1 className="page-title">{t.org.title(org.label)}</h1>
        <OrgMenu value={org.id} orgs={me.orgs} onChoose={onChoose} />
      </div>
      {tree.focus === undefined ? null : (
        <p className="oc-up">
          <Link to="/org" search={{ org: org.id }}>{t.org.whole}</Link>
        </p>
      )}
      {tree.children.length === 0 && tree.focus !== undefined ? (
        <p className="empty">{t.org.emptyDepartment}</p>
      ) : (
        <OrgChartViewer resetKey={`${org.id}/${unit ?? '' /* stand-in-ok(routing): the key that resets the chart's view when the department changes */}`} fileName={`org-chart-${org.id}${unit === undefined ? '' : `-${unit}`}`}>
          <OrgTree tree={tree} org={org.id} orgLabel={org.label} />
        </OrgChartViewer>
      )}
    </>
  )
}

/** The chart the address names: every org, or one. It reads every team's view, which suspends it until they are read; a team that keeps failing is a box that says so. */
function OrgChart({ org, unit }: OrgChartPageProps) {
  const navigate = useNavigate()
  const { me, teams, loading } = useSuspenseTeamStates()
  const onChoose = (next: string): void => void navigate({ to: '/org', search: { org: next } })
  const selected = me.orgs.find((one) => one.id === org) ?? me.orgs[0]
  if (me.orgs.length > 1 && (org === undefined || org === ALL_ORGS)) return <AllOrgsChart me={me} teams={teams} loading={loading} onChoose={onChoose} />
  return selected === undefined ? null : <OneOrgChart me={me} teams={teams} loading={loading} org={selected} unit={unit} onChoose={onChoose} />
}

export function OrgChartPage({ org, unit }: OrgChartPageProps) {
  const { t } = useI18n()
  // While another org or department is read, the chart that is shown stays instead of giving way to the skeleton.
  const shownOrg = useDeferredValue(org)
  const shownUnit = useDeferredValue(unit)
  return (
    <Region what={t.loading.what.orgChart} skeleton={<OrgChartSkeleton />}>
      <OrgChart org={shownOrg} unit={shownUnit} />
    </Region>
  )
}
