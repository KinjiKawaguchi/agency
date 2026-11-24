import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { TEAM_KEY, useTeams } from '../api/queries'
import { useI18n } from '../i18n'
import { ReplicaNotice } from '../shared/ReplicaNotice'
import { useToast } from '../shared/Toasts'
import { Briefing } from './Briefing'
import { draftOf, withDraft, withoutDraft, type Draft, type Drafts } from './drafts'
import { canDecide, openItems, resolveSelection, situationOf, stepSelection, type DeskItem } from './items'
import { Paper } from './Paper'
import { Pile } from './Pile'
import { useDeskKeys } from './useDeskKeys'
import './desk.css'

/** How long a pressed seal stays visible before the answered item leaves the pile. */
export const SEAL_HOLD_MS = 1600

interface DeskPageProps {
  /** The open item's key, kept in the address so a link opens the same document. */
  selected: string | undefined
  onSelect: (key: string) => void
  sealHoldMs?: number
}

export function DeskPage({ selected: requested, onSelect, sealHoldMs = SEAL_HOLD_MS }: DeskPageProps) {
  const { t } = useI18n()
  const { me, teams, isLoading, error } = useTeams()
  const api = useApi()
  const queryClient = useQueryClient()
  const toast = useToast()
  const [drafts, setDrafts] = useState<Drafts>(new Map())
  const [sealed, setSealed] = useState<ReadonlyMap<string, string>>(new Map())
  const [sending, setSending] = useState<ReadonlySet<string>>(new Set())
  const timers = useRef<number[]>([])
  useEffect(() => () => timers.current.forEach(window.clearTimeout), [])

  const items = useMemo(() => openItems(teams), [teams])
  const selectedKey = resolveSelection(items, requested)
  const current = items.find((item) => item.key === selectedKey)

  const send = useCallback(
    async (item: DeskItem) => {
      if (!canDecide(item)) return
      const draft = draftOf(drafts, item.key)
      const hasAnswer = draft.text.trim() !== '' || draft.choice !== null
      if (!hasAnswer) return toast(item.request.kind === 'pendingDecision' ? t.desk.paper.needDecision : t.desk.paper.needAnswer, 'error')
      setSending((keys) => new Set([...keys, item.key]))
      try {
        const result = await api.decide({ org: item.view.org, team: item.view.team, answerWith: item.request.answerWith, text: draft.text.trim(), ...(draft.choice === null ? {} : { choice: draft.choice }) })
        setDrafts((current) => withoutDraft(current, item.key))
        setSealed((current) => new Map([...current, [item.key, new Date().toISOString()]]))
        toast(result.outcome === 'queued' ? [t.desk.paper.queued, result.message].filter(Boolean).join(' ') : t.desk.paper.sent(item.drafter?.label ?? item.request.from))
        timers.current.push(window.setTimeout(() => void queryClient.invalidateQueries({ queryKey: TEAM_KEY }), sealHoldMs))
      } catch (failure) {
        const reason = failure instanceof Error ? failure.message : String(failure)
        toast(failure instanceof ApiError && failure.status === 409 ? t.desk.paper.refused(reason) : t.desk.paper.sendFailed(reason), 'error')
      } finally {
        setSending((keys) => new Set([...keys].filter((key) => key !== item.key)))
      }
    },
    [api, drafts, queryClient, sealHoldMs, t, toast],
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
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : t.app.unreachable(error.message)}</p>
  }

  const orgLabelOf = (id: string): string => me.orgs.find((org) => org.id === id)?.label ?? id
  const situation = situationOf(teams, items)
  return (
    <>
      {error === null ? null : <p className="empty" role="alert">{t.app.unreachable(error.message)}</p>}
      {teams.map((team) => (
        <ReplicaNotice key={`${team.view.org}/${team.view.team}`} source={team.source} />
      ))}
      <Briefing me={me} teams={teams} items={items} situation={situation} now={new Date()} />
      <div className="desk">
        <div className="desk-left">
          {isLoading && items.length === 0 ? <p className="muted">{t.app.loading}</p> : <Pile items={items} selected={selectedKey} orgLabelOf={me.orgs.length > 1 ? orgLabelOf : null} onSelect={onSelect} />}
        </div>
        {current === undefined ? (
          <article className="paper blank"><p>{t.desk.paper.blank}</p></article>
        ) : (
          <Paper
            item={current}
            orgLabel={orgLabelOf(current.view.org)}
            viewerLabel={me.account.label}
            draft={draftOf(drafts, current.key)}
            sealedAt={sealed.get(current.key) ?? null}
            isSending={sending.has(current.key)}
            onChange={(change: Partial<Draft>) => setDrafts((now) => withDraft(now, current.key, change))}
            onSend={() => void send(current)}
          />
        )}
      </div>
    </>
  )
}
