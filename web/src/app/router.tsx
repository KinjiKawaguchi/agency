import { OutcomesPage } from '../outcomes/OutcomesPage'
import { OUTCOMES_PERIODS } from '../outcomes/useOutcomesAccess'
import { Outlet, createRootRoute, createRoute, createRouter, redirect, useRouterState, type RouterHistory } from '@tanstack/react-router'
import { CostsPage } from '../costs/CostsPage'
import { EvaluationPage, PERIODS } from '../evaluation/EvaluationPage'
import { LivePage } from '../live/LivePage'
import { DevicesPage } from '../device/DevicesPage'
import { DeskPage } from '../desk/DeskPage'
import { OrgChartPage } from '../org/OrgChartPage'
import { OverviewPage } from '../overview/OverviewPage'
import { PeoplePage } from '../people/PeoplePage'
import { ClaimPage } from '../claim/ClaimPage'
import { AddDeviceNotice } from '../device/AddDeviceNotice'
import { Notices } from '../notifications/Notices'
import { SignInExpiredBanner } from '../shared/SignInExpired'
import { AppBar } from './AppBar'

/** The claim page is a person's first page, before there is anything of the app for them: it has none of the app's bar and notices. */
const isClaimPath = (path: string): boolean => path === '/claim' || path.startsWith('/claim/')

function Shell() {
  const path = useRouterState({ select: (state) => state.location.pathname })
  if (isClaimPath(path)) {
    return (
      <main className="page">
        <Outlet />
      </main>
    )
  }
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

const outcomesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/outcomes',
  validateSearch: (search: Record<string, unknown>): { org?: string; days?: number } => ({
    ...(nonEmpty(search.org) === undefined ? {} : { org: nonEmpty(search.org) }),
    ...(OUTCOMES_PERIODS.find((one) => one === Number(search.days)) === undefined ? {} : { days: Number(search.days) }),
  }),
  component: function OutcomesRoute() {
    const { org, days } = outcomesRoute.useSearch()
    const navigate = outcomesRoute.useNavigate()
    return <OutcomesPage org={org} days={days} onChange={(next) => void navigate({ search: next, replace: true })} />
  },
})

const liveRoute = createRoute({ getParentRoute: () => rootRoute, path: '/live', component: LivePage })

const devicesRoute = createRoute({ getParentRoute: () => rootRoute, path: '/devices', component: () => <DevicesPage /> })

// `/claim/continue` is where the public landing page sends the person after the sign-in; `/claim` itself is the landing page,
// which the node serves as a static file, and which this route stands in for where the node does not (a dev server).
const claimRoute = createRoute({ getParentRoute: () => rootRoute, path: '/claim', component: ClaimPage })
const claimContinueRoute = createRoute({ getParentRoute: () => rootRoute, path: '/claim/continue', component: ClaimPage })

const routeTree = rootRoute.addChildren([indexRoute, claimRoute, claimContinueRoute, deskRoute, overviewRoute, peopleRoute, orgRoute, costsRoute, evaluationRoute, liveRoute, outcomesRoute, devicesRoute])

export function createAppRouter(history?: RouterHistory) {
  return createRouter({ routeTree, ...(history === undefined ? {} : { history }) })
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>
  }
}
