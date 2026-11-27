import { OUTCOMES_PERIODS } from '../outcomes/useOutcomesAccess'
import { Navigate, Outlet, createRootRoute, createRoute, createRouter, redirect, useRouterState, type RouterHistory } from '@tanstack/react-router'
import { PERIODS } from '../evaluation/periods'
import { DevicesPage } from '../device/DevicesPage'
import { DeskPage } from '../desk/DeskPage'
import { ClaimPage } from '../claim/ClaimPage'
import { AddDeviceNotice } from '../device/AddDeviceNotice'
import { isGuestPage, useStanding } from '../guest/useStanding'
import { Notices } from '../notifications/Notices'
import { SignInExpiredBanner } from '../shared/SignInExpired'
import { Loading, Skeleton } from '../shared/Skeleton'
import { SendingTray } from '../outbox/SendingTray'
import { AppBar } from './AppBar'
import { lazyPage } from './lazyPage'

// The desk and the devices are what a person opens first, and what a hosted member has; the other pages are fetched when they are opened.
const OverviewPage = lazyPage(() => import('../overview/OverviewPage').then((page) => ({ default: page.OverviewPage })))
const PeoplePage = lazyPage(() => import('../people/PeoplePage').then((page) => ({ default: page.PeoplePage })))
const OrgChartPage = lazyPage(() => import('../org/OrgChartPage').then((page) => ({ default: page.OrgChartPage })))
const CostsPage = lazyPage(() => import('../costs/CostsPage').then((page) => ({ default: page.CostsPage })))
const EvaluationPage = lazyPage(() => import('../evaluation/EvaluationPage').then((page) => ({ default: page.EvaluationPage })))
const OutcomesPage = lazyPage(() => import('../outcomes/OutcomesPage').then((page) => ({ default: page.OutcomesPage })))
const LivePage = lazyPage(() => import('../live/LivePage').then((page) => ({ default: page.LivePage })))

/** The claim page is a person's first page, before there is anything of the app for them: it has none of the app's bar and notices. */
const isClaimPath = (path: string): boolean => path === '/claim' || path.startsWith('/claim/')

function Shell() {
  const path = useRouterState({ select: (state) => state.location.pathname })
  return isClaimPath(path) ? (
    <main className="page">
      <Outlet />
    </main>
  ) : (
    <AppShell path={path} />
  )
}

function AppShell({ path }: { path: string }) {
  const standing = useStanding()
  // The page on screen can still be the one left while the location is already the new one, so it is asked for itself.
  const shown = useRouterState({ select: (state) => state.matches.at(-1)?.pathname ?? state.location.pathname })
  // A page of the owner's would be refused to a guest, so it is not opened: they have the desk and their devices.
  if (standing.status === 'guest' && !isGuestPage(path)) return <Navigate to="/desk" replace />
  // The desk and the devices are the guest's own, so they open at once; the owner's pages wait to learn who is asking.
  const isWaiting = (standing.status === 'loading' || standing.status === 'guest') && !isGuestPage(shown)
  return (
    <>
      <AppBar />
      <Notices />
      <main className="page">
        <SignInExpiredBanner />
        <AddDeviceNotice />
        {isWaiting ? (
          <Loading>
            <Skeleton kind="title" />
            <Skeleton />
          </Loading>
        ) : (
          <Outlet />
        )}
      </main>
      <SendingTray />
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
