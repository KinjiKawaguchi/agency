import { Link } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { ApiError } from '../api/client'
import { CannotSignError, useSignDecision, useSigningState } from '../device/useDecide'
import { SignInExpiredError } from '../api/session'
import type { Messages } from '../i18n/en'
import { useTeams } from '../api/queries'
import { requestKey, teamKey } from '../api/teams'
import { useOutbox } from '../outbox/OutboxProvider'
import { failureText } from '../api/failure'
import { useI18n } from '../i18n'
import { RecoveryNotice } from '../recovery/RecoveryNotice'
import { RootChangesNotice } from '../rootchange/RootChangesNotice'
import { ReplicaNotice } from '../shared/ReplicaNotice'
import { useToast } from '../shared/Toasts'
import { Briefing } from './Briefing'
import { draftOf, loadDrafts, saveDrafts, withDraft, withoutDraft, type Draft, type Drafts } from './drafts'
import { canDecide, openItems, resolveSelection, situationOf, stepSelection, type DeskItem } from './items'
import { Paper } from './Paper'
import { Pile, PileSkeleton } from './Pile'
import { asGiven } from '../shared/printable'
import { RecentDecisions } from './RecentDecisions'
import { useDeskKeys } from './useDeskKeys'
import './desk.css'

/** The option the draft picked, with its label as the node gave it (what is sent back is never the shown form). */
function chosenOf(item: DeskItem, draft: Draft): { choice?: string; choiceLabel?: string } {
  if (draft.choice === null) return {}
  const option = item.request.options.find((one) => one.key === draft.choice)
  const label = option === undefined ? undefined : asGiven(option, 'text')
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
}

export function DeskPage({ selected: requested, onSelect, team: onlyTeam }: DeskPageProps) {
  const { t } = useI18n()
  const { me, teams: everyTeam, isLoading, error } = useTeams()
  const scope = onlyTeam === undefined ? undefined : everyTeam.find((team) => teamKey(team) === onlyTeam)
  const teams = useMemo(() => (scope === undefined ? everyTeam : [scope]), [everyTeam, scope])
  const sign = useSignDecision()
  const outbox = useOutbox()
  const toast = useToast()
  const [drafts, setDrafts] = useState<Drafts>(loadDrafts)
  const [sending, setSending] = useState<ReadonlySet<string>>(new Set())
  useEffect(() => saveDrafts(drafts), [drafts])
  // A decision the owner gives up before the node took it comes back on the pile with what was typed.
  useEffect(() => outbox.onDiscard((entry) => setDrafts((now) => withDraft(now, entry.itemKey, entry.draft))), [outbox])

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
      const hasAnswer = draft.text.trim() !== '' || draft.choice !== null
      if (!hasAnswer) return toast(item.request.kind === 'pendingDecision' ? t.desk.paper.needDecision : t.desk.paper.needAnswer, 'error')
      setSending((keys) => new Set([...keys, item.key]))
      try {
        const chosen = chosenOf(item, draft)
        const signed = await sign({ org: item.view.org, team: item.view.team, answerWith: item.request.answerWith, title: asGiven(item.request, 'title') ?? item.request.title, text: draft.text.trim(), ...chosen })
        const chosenLabel = chosen.choice === undefined ? undefined : item.request.options.find((one) => one.key === chosen.choice)?.text
        // The decision is signed and kept: the request leaves the pile and the next one is open at once, and the tray sends it.
        outbox.enqueue({
          id: signed.envelope.body.id,
          org: item.view.org,
          team: item.view.team,
          itemKey: requestKey(item.view, item.request),
          itemTitle: item.request.title,
          ...(chosenLabel === undefined ? {} : { chosenLabel }),
          text: draft.text.trim(),
          draft,
          envelope: signed.envelope,
          title: signed.title,
          ...(signed.choiceLabel === undefined ? {} : { choiceLabel: signed.choiceLabel }),
          createdAt: new Date().toISOString(),
          attempts: 0,
          phase: 'sending',
        })
        setDrafts((current) => withoutDraft(current, item.key))
        const after = stepSelection(items, item.key, 1)
        const next = after !== item.key ? after : stepSelection(items, item.key, -1)
        if (next !== undefined && next !== item.key) onSelect(next)
      } catch (failure) {
        const reason = failure instanceof Error ? failure.message : String(failure)
        toast(sendFailureText(t, failure, reason), 'error')
      } finally {
        setSending((keys) => new Set([...keys].filter((key) => key !== item.key)))
      }
    },
    [drafts, items, onSelect, outbox, sign, t, toast],
  )

  useDeskKeys({
    onMove: (step) => {
      const next = stepSelection(items, selectedKey, step)
      if (next !== undefined) onSelect(next)
    },
    onPick: (position) => {
      const option = current !== undefined && canDecide(current) ? current.request.options[position - 1] : undefined
      if (current !== undefined && option !== undefined) setDrafts((now) => withDraft(now, current.key, { choice: option.key }))
    },
    onSend: () => {
      if (current !== undefined && !sending.has(current.key)) void send(current)
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
            sealedAt={null}
            wasComment={false}
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
