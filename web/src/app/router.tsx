import { Outlet, createRootRoute, createRoute, createRouter, redirect, type RouterHistory } from '@tanstack/react-router'
import { CostsPage } from '../costs/CostsPage'
import { EvaluationPage, PERIODS } from '../evaluation/EvaluationPage'
import { DevicesPage } from '../device/DevicesPage'
import { DeskPage } from '../desk/DeskPage'
import { OrgChartPage } from '../org/OrgChartPage'
import { OverviewPage } from '../overview/OverviewPage'
import { PeoplePage } from '../people/PeoplePage'
import { AddDeviceNotice } from '../device/AddDeviceNotice'
import { Notices } from '../notifications/Notices'
import { SignInExpiredBanner } from '../shared/SignInExpired'
import { AppBar } from './AppBar'

function Shell() {
  return (
    <>
      <AppBar />
      <Notices />
      <main className="page">
        <SignInExpiredBanner />
        <AddDeviceNotice />
        <Outlet />
      </main>
    </>
  )
}

const rootRoute = createRootRoute({ component: Shell })

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: '/desk', replace: true })
  },
})

const deskRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/desk',
  validateSearch: (search: Record<string, unknown>): { item?: string; team?: string } => ({
    ...(typeof search.item === 'string' && search.item !== '' ? { item: search.item } : {}),
    ...(typeof search.team === 'string' && search.team !== '' ? { team: search.team } : {}),
  }),
  component: function DeskRoute() {
    const { item, team } = deskRoute.useSearch()
    const navigate = deskRoute.useNavigate()
    return <DeskPage selected={item} team={team} onSelect={(key) => void navigate({ search: (previous) => ({ ...previous, item: key }), replace: true })} />
  },
})

const overviewRoute = createRoute({ getParentRoute: () => rootRoute, path: '/overview', component: OverviewPage })

const peopleRoute = createRoute({ getParentRoute: () => rootRoute, path: '/people', component: PeoplePage })
const nonEmpty = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

const orgRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/org',
  validateSearch: (search: Record<string, unknown>): { org?: string; unit?: string } => ({
    ...(nonEmpty(search.org) === undefined ? {} : { org: nonEmpty(search.org) }),
    ...(nonEmpty(search.unit) === undefined ? {} : { unit: nonEmpty(search.unit) }),
  }),
  component: function OrgRoute() {
    const { org, unit } = orgRoute.useSearch()
    return <OrgChartPage org={org} unit={unit} />
  },
})
const costsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/costs', component: CostsPage })

const evaluationRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/evaluation',
  validateSearch: (search: Record<string, unknown>): { org?: string; days?: number } => ({
    ...(nonEmpty(search.org) === undefined ? {} : { org: nonEmpty(search.org) }),
    ...(PERIODS.find((one) => one === Number(search.days)) === undefined ? {} : { days: Number(search.days) }),
  }),
  component: function EvaluationRoute() {
    const { org, days } = evaluationRoute.useSearch()
    const navigate = evaluationRoute.useNavigate()
    return <EvaluationPage org={org} days={days} onChange={(next) => void navigate({ search: next, replace: true })} />
  },
})

const devicesRoute = createRoute({ getParentRoute: () => rootRoute, path: '/devices', component: () => <DevicesPage /> })

const routeTree = rootRoute.addChildren([indexRoute, deskRoute, overviewRoute, peopleRoute, orgRoute, costsRoute, evaluationRoute, devicesRoute])

export function createAppRouter(history?: RouterHistory) {
  return createRouter({ routeTree, ...(history === undefined ? {} : { history }) })
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>
  }
}
