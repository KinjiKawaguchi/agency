import { useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ApiError } from '../api/client'
import { CannotSignError, useDecide, useSigningState } from '../device/useDecide'
import { SignInExpiredError } from '../api/session'
import type { Messages } from '../i18n/en'
import { DECISIONS_KEY, TEAM_KEY, useTeams } from '../api/queries'
import { teamKey } from '../api/teams'
import { failureText } from '../api/failure'
import { useI18n } from '../i18n'
import { RecoveryNotice } from '../recovery/RecoveryNotice'
import { RootChangesNotice } from '../rootchange/RootChangesNotice'
import { ReplicaNotice } from '../shared/ReplicaNotice'
import { printable } from '../shared/printable'
import { useToast } from '../shared/Toasts'
import { Briefing } from './Briefing'
import { draftOf, loadDrafts, saveDrafts, withDraft, withoutDraft, type Draft, type Drafts } from './drafts'
import { isComment } from './comment'
import { canDecide, openItems, resolveSelection, situationOf, stepSelection, type DeskItem } from './items'
import { Paper } from './Paper'
import { Pile, PileSkeleton } from './Pile'
import { RecentDecisions } from './RecentDecisions'
import { useDeskKeys } from './useDeskKeys'
import './desk.css'

/** How long a pressed seal stays visible before the answered item leaves the pile. */
export const SEAL_HOLD_MS = 1600

/** The option the draft picked, with the label the viewer saw on it. */
function chosenOf(item: DeskItem, draft: Draft): { choice?: string; choiceLabel?: string } {
  if (draft.choice === null) return {}
  const label = item.request.options.find((option) => option.key === draft.choice)?.text
  return { choice: draft.choice, ...(label === undefined ? {} : { choiceLabel: label }) }
}

function sendFailureText(t: Messages, failure: unknown, reason: string): string {
  if (failure instanceof SignInExpiredError) return t.app.signInExpired.message
  if (failure instanceof CannotSignError) return failure.reason === 'unsupported' ? t.desk.paper.cannotSign.unsupported : t.desk.paper.cannotSign.prompt
  return failure instanceof ApiError && failure.status === 409 ? t.desk.paper.refused(reason) : t.desk.paper.sendFailed(reason)
}

interface DeskPageProps {
  /** The open item's key, kept in the address so a link opens the same document. */
  selected: string | undefined
  onSelect: (key: string) => void
  /** Shows only this team's requests (`org/team`), as a link from the overview does. */
  team?: string | undefined
  sealHoldMs?: number
}

export function DeskPage({ selected: requested, onSelect, team: onlyTeam, sealHoldMs = SEAL_HOLD_MS }: DeskPageProps) {
  const { t } = useI18n()
  const { me, teams: everyTeam, isLoading, error } = useTeams()
  const scope = onlyTeam === undefined ? undefined : everyTeam.find((team) => teamKey(team) === onlyTeam)
  const teams = useMemo(() => (scope === undefined ? everyTeam : [scope]), [everyTeam, scope])
  const decide = useDecide()
  const queryClient = useQueryClient()
  const toast = useToast()
  const [drafts, setDrafts] = useState<Drafts>(loadDrafts)
  const [sealed, setSealed] = useState<ReadonlyMap<string, { at: string; isComment: boolean }>>(new Map())
  const [sending, setSending] = useState<ReadonlySet<string>>(new Set())
  const timers = useRef<number[]>([])
  useEffect(() => () => timers.current.forEach(window.clearTimeout), [])
  useEffect(() => saveDrafts(drafts), [drafts])

  /** Opens the document; where the paper sits below the pile, brings it into view so the tap visibly does something. */
  const pick = useCallback(
    (key: string) => {
      onSelect(key)
      if (window.matchMedia?.('(max-width: 960px)').matches) window.requestAnimationFrame(() => document.querySelector('article.paper')?.scrollIntoView?.({ behavior: 'smooth', block: 'start' }))
    },
    [onSelect],
  )

  const items = useMemo(() => openItems(teams), [teams])
  const selectedKey = resolveSelection(items, requested)
  const current = items.find((item) => item.key === selectedKey)
  const signing = useSigningState(current?.view.org ?? '' /* stand-in-ok(routing): which org signs for the open request; there is none when no request is open */)

  const send = useCallback(
    async (item: DeskItem) => {
      if (!canDecide(item)) return
      const draft = draftOf(drafts, item.key)
      const asComment = isComment(item.request.options.length, draft)
      const hasAnswer = draft.text.trim() !== '' || draft.choice !== null
      if (!hasAnswer) return toast(item.request.kind === 'pendingDecision' ? t.desk.paper.needDecision : t.desk.paper.needAnswer, 'error')
      setSending((keys) => new Set([...keys, item.key]))
      try {
        const result = await decide({ org: item.view.org, team: item.view.team, answerWith: item.request.answerWith, title: item.request.title, text: draft.text.trim(), ...chosenOf(item, draft) })
        setDrafts((current) => withoutDraft(current, item.key))
        setSealed((current) => new Map([...current, [item.key, { at: new Date().toISOString(), isComment: asComment }]]))
        toast(result.outcome === 'queued' ? [t.desk.paper.queued, result.message].filter(Boolean).join(' ') : (asComment ? t.desk.paper.sentComment : t.desk.paper.sent)(item.drafter?.label ?? printable(item.request.from)))
        void queryClient.invalidateQueries({ queryKey: DECISIONS_KEY })
        timers.current.push(window.setTimeout(() => void queryClient.invalidateQueries({ queryKey: TEAM_KEY }), sealHoldMs))
      } catch (failure) {
        const reason = failure instanceof Error ? failure.message : String(failure)
        toast(sendFailureText(t, failure, reason), 'error')
      } finally {
        setSending((keys) => new Set([...keys].filter((key) => key !== item.key)))
      }
    },
    [decide, drafts, queryClient, sealHoldMs, t, toast],
  )

  useDeskKeys({
    onMove: (step) => {
      const next = stepSelection(items, selectedKey, step)
      if (next !== undefined) onSelect(next)
    },
    onPick: (position) => {
      const option = current !== undefined && canDecide(current) && !sealed.has(current.key) ? current.request.options[position - 1] : undefined
      if (current !== undefined && option !== undefined) setDrafts((now) => withDraft(now, current.key, { choice: option.key }))
    },
    onSend: () => {
      if (current !== undefined && !sealed.has(current.key) && !sending.has(current.key)) void send(current)
    },
  })

  if (me === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : failureText(t, error)}</p>
  }

  const orgLabelOf = (id: string): string => me.orgs.find((org) => org.id === id)?.label ?? id
  const situation = situationOf(teams, items)
  return (
    <>
      <RecoveryNotice me={me} />
      <RootChangesNotice me={me} />
      {error === null ? null : <p className="empty" role="alert">{failureText(t, error)}</p>}
      {teams.map((team) => (
        <ReplicaNotice key={`${team.view.org}/${team.view.team}`} source={team.source} />
      ))}
      {scope === undefined ? null : (
        <p className="desk-scope">
          {t.desk.pile.scope.only(scope.view.label)} <Link to="/desk">{t.desk.pile.scope.all}</Link>
        </p>
      )}
      <Briefing me={me} teams={teams} items={items} situation={situation} now={new Date()} isLoading={isLoading} />
      <div className="desk">
        <div className="desk-pile">
          {isLoading && items.length === 0 ? <PileSkeleton /> : <Pile items={items} selected={selectedKey} orgLabelOf={me.orgs.length > 1 ? orgLabelOf : null} onSelect={pick} />}
          {isLoading && items.length > 0 ? <PileSkeleton rows={1} /> : null}
        </div>
        {current === undefined ? null : (
          <Paper
            item={current}
            orgLabel={orgLabelOf(current.view.org)}
            viewerLabel={me.account.label}
            draft={draftOf(drafts, current.key)}
            sealedAt={sealed.get(current.key)?.at ?? null}
            wasComment={sealed.get(current.key)?.isComment ?? false /* stand-in-ok(device): what this device sealed in this page, not the node's */}
            isSending={sending.has(current.key)}
            signing={signing}
            onChange={(change: Partial<Draft>) => setDrafts((now) => withDraft(now, current.key, change))}
            onSend={() => void send(current)}
          />
        )}
        <RecentDecisions me={me} />
      </div>
    </>
  )
}
