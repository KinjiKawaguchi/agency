import { render, screen, waitFor, within } from '@testing-library/react'
import type { ParticipantEvaluation } from '../api/evaluationApi'
import { I18nProvider } from '../i18n'
import { createTestNode } from '../test/testDevice'
import { renderApp } from '../test/render'
import { ParticipantsSection } from './ParticipantsSection'

const person = (over: Partial<ParticipantEvaluation> = {}): ParticipantEvaluation => ({
  name: 'backend',
  busySeconds: 30_600,
  waitingSeconds: 2_400,
  idleSeconds: 4_800,
  costUsd: 8.45,
  tokens: { input: 120_000, output: 45_000, cacheRead: 2_400_000, cacheWrite: 180_000 },
  merged: 3,
  costPerMergedPrUsd: 2.82,
  ...over,
})

const draw = (participants: ParticipantEvaluation[], unattributedMerged?: number, locale: 'en' | 'ja' = 'en') =>
  render(
    <I18nProvider initial={locale}>
      <ParticipantsSection participants={participants} unattributedMerged={unattributedMerged} />
    </I18nProvider>,
  )

describe('the participants of a team', () => {
  it('shows each participant’s time with a session alive in three spans, whose waiting is said to be at least', () => {
    draw([person()])
    const row = within(screen.getByRole('heading', { level: 4, name: 'backend' }).closest('li')!)
    expect(row.getByRole('img')).toHaveAttribute('aria-label', 'Working 8.5h, waiting at least 40m, idle 1.3h')
    expect(row.getByText(/Waiting for a decision \(at least\) 40m/)).toBeInTheDocument()
    expect(row.getByText(/Working 8.5h/)).toBeInTheDocument()
    const widths = [...document.querySelectorAll('.ev-span')].map((span) => Number((span as HTMLElement).style.flexGrow))
    expect(widths.reduce((sum, share) => sum + share, 0)).toBeCloseTo(1, 5)
    expect(widths[0]).toBeGreaterThan(widths[1] ?? 0)
  })

  it('shows the cost with its tokens, the merged pull requests, and the cost for each of them', () => {
    draw([person()])
    expect(screen.getByText('$8.45')).toBeInTheDocument()
    expect(screen.getByText('2,745,000 tokens')).toBeInTheDocument()
    expect(screen.getByText('Merged').nextElementSibling).toHaveTextContent('3')
    expect(screen.getByText('$2.82')).toBeInTheDocument()
  })

  it('says a participant who merged nothing has no cost per pull request, not $0', () => {
    draw([person({ name: 'reviewer', merged: 0, costPerMergedPrUsd: undefined })])
    expect(screen.getByText('None merged')).toBeInTheDocument()
    expect(screen.queryByText('$0')).not.toBeInTheDocument()
    expect(screen.getByText('Merged').nextElementSibling).toHaveTextContent('0')
  })

  it('draws no bar, and says so, for a participant with no session alive in the period', () => {
    draw([person({ busySeconds: 0, waitingSeconds: 0, idleSeconds: 0 })])
    expect(screen.getByText('No session was alive in this period.')).toBeInTheDocument()
    expect(document.querySelector('.ev-timebar')).toBeNull()
  })

  it('names the merged pull requests no participant is credited with, and only when there are some', () => {
    draw([person()], 2)
    expect(screen.getByText(/2 merged pull requests are not attributed to a participant/)).toBeInTheDocument()
    expect(screen.getByText(/add up to the team’s/)).toBeInTheDocument()
  })

  it('says nothing of it for none or when the node left it out', () => {
    const { unmount } = draw([person()], 0)
    expect(screen.queryByText(/not attributed/)).not.toBeInTheDocument()
    unmount()
    draw([person()])
    expect(screen.queryByText(/not attributed/)).not.toBeInTheDocument()
  })

  it('says one pull request in the singular', () => {
    draw([person()], 1)
    expect(screen.getByText(/1 merged pull request is not attributed to a participant/)).toBeInTheDocument()
  })

  it('reads in Japanese', () => {
    draw([person()], 2, 'ja')
    expect(screen.getByText('参加者')).toBeInTheDocument()
    expect(screen.getByRole('img')).toHaveAttribute('aria-label', '作業中 8.5時間、判断待ち 40分 以上、待機中 1.3時間')
    expect(screen.getByText(/2 件は、参加者に結び付けられていません/)).toBeInTheDocument()
  })
})

describe('on the evaluation page', () => {
  it('shows each team’s participants when the node gives them to its owner', async () => {
    renderApp({ path: '/evaluation', node: createTestNode({ evaluation: true, participants: true }), enrolled: true })
    expect(await screen.findAllByRole('heading', { level: 4, name: 'backend' })).not.toHaveLength(0)
    expect(screen.getAllByRole('heading', { level: 4, name: 'reviewer' }).length).toBeGreaterThan(0)
    expect(screen.getAllByText('None merged').length).toBeGreaterThan(0)
  })

  it('shows nothing of them, and no error or hint, when the answer has none (the viewer is not the team’s owner)', async () => {
    renderApp({ path: '/evaluation', node: createTestNode({ evaluation: true }), enrolled: true })
    await screen.findAllByRole('heading', { level: 2, name: /team$/ })
    await waitFor(() => expect(document.querySelector('.ev-team .card')).not.toBeNull())
    expect(screen.queryByRole('heading', { level: 4 })).not.toBeInTheDocument()
    expect(screen.queryByText('Participants')).not.toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByText(/owner/i)).not.toBeInTheDocument()
  })
})
