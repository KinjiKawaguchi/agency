import { render, screen, within } from '@testing-library/react'
import type { ActivityEvent } from '../api/activityApi'
import type { ParticipantEvaluation } from '../api/evaluationApi'
import { sampleTeamView, type TeamView } from '../api/teamview'
import { ParticipantsSection } from '../evaluation/ParticipantsSection'
import { I18nProvider } from '../i18n'
import { Feed } from '../live/Feed'
import { GUEST_TEAM, guestsOf, keyOf } from '../live/graph'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'
import { withRequestFor } from '../test/views'

// A name of the team's runtime, drawn where a person's name would be, has each character that hides or reorders a line as a mark.
const RLO = String.fromCodePoint(0x202e)
const RAW = `gh${RLO}ost`
const SHOWN = 'gh?ost'

const proposalTitle = 'Add a second frontend agent'

function viewFrom(name: string): TeamView {
  return { ...sampleTeamView, requests: sampleTeamView.requests.map((request) => (request.title === proposalTitle ? { ...request, from: name } : request)) }
}

const theDocument = (): string => document.body.textContent ?? ''

describe('the name of who drafted a request, when the team has no such person', () => {
  it('is a mark on the pile and in the markup of the paper, and never the character itself', async () => {
    renderApp({ node: createTestNode({ view: viewFrom(RAW) }), path: '/desk', enrolled: true })
    await screen.findAllByText(proposalTitle)
    const row = (await screen.findAllByText(proposalTitle)).map((one) => one.closest('button')).find((one) => one !== null) as HTMLElement
    expect(within(row).getByText(SHOWN)).toBeInTheDocument()
    // The paper's stamp says it in a title and an initial, not in a line of text: the markup, attributes included, is what is looked at.
    expect(document.body.innerHTML).toContain(SHOWN)
    expect(document.body.innerHTML).not.toContain(RLO)
  })
})

describe('whom a request is for, when the team has no such person', () => {
  it('is a mark on the paper of a request that is another’s', async () => {
    const view = withRequestFor(`fi${RLO}nance`)
    renderApp({ node: createTestNode({ view }), path: '/desk?item=acme:app:r-20261124-120000-e5f6', enrolled: true })
    await screen.findAllByText('Should the nightly export include archived projects?')
    expect(await screen.findAllByText(/fi\?nance/)).not.toHaveLength(0)
    expect(theDocument()).not.toContain(RLO)
  })
})

describe('a message of the thread of a request', () => {
  it('names who wrote it by a mark, when the team has no such person', async () => {
    const view: TeamView = {
      ...sampleTeamView,
      requests: sampleTeamView.requests.map((request) => (request.title === proposalTitle ? { ...request, thread: [{ at: '2026-11-24T10:00:00Z', from: RAW, text: 'A note.' }] } : request)),
    }
    renderApp({ node: createTestNode({ view }), path: '/desk', enrolled: true })
    expect(await screen.findAllByText(/gh\?ost/)).not.toHaveLength(0)
    expect(theDocument()).not.toContain(RLO)
  })
})

const message = (from: string, to: string): ActivityEvent => ({ id: 'm-1', at: '2026-11-24T10:00:00Z', kind: 'message', from, to, summary: '' }) as ActivityEvent

describe('who a message of the live feed is between', () => {
  it('is marks, where the names are the runtime’s', () => {
    render(
      <I18nProvider initial="en">
        <Feed events={[{ org: 'acme', team: 'app', event: message(RAW, `to${RLO}o`) }]} fresh={[]} teamLabels={new Map([['acme/app', 'App']])} isReading={false} hasUnread={false} />
      </I18nProvider>,
    )
    expect(document.body.textContent).toContain('gh?ost')
    expect(document.body.textContent).toContain('to?o')
    expect(document.body.textContent).not.toContain(RLO)
  })
})

describe('a guest of the picture of the live view', () => {
  it('is named by a mark, and is still found by the name it has', () => {
    const resolve = (org: string, _team: string, name: string) => keyOf({ org, team: GUEST_TEAM, id: name })
    const guests = guestsOf([{ org: 'acme', team: 'app', event: message(RAW, 'bob') }], resolve, { now: Date.parse('2026-11-24T10:01:00Z') })
    const found = guests.shown.find((one) => one.id === RAW)
    expect(found?.label).toBe(SHOWN)
  })
})

describe('the name of a participant of the evaluation', () => {
  it('is a mark', () => {
    const participant = { name: RAW, busySeconds: 1, waitingSeconds: 1, idleSeconds: 1, costUsd: 1, tokens: { input: 1, output: 1, cacheRead: 1, cacheWrite: 1 }, merged: 0 } as ParticipantEvaluation
    render(
      <I18nProvider initial="en">
        <ParticipantsSection participants={[participant]} unattributedMerged={undefined} />
      </I18nProvider>,
    )
    expect(screen.getByRole('heading', { level: 4, name: SHOWN })).toBeInTheDocument()
    expect(document.body.textContent).not.toContain(RLO)
  })
})
