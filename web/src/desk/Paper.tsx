import { canDecide, blockedPeople, type DeskItem } from './items'
import { personOf } from '../people/viewer'
import { type Draft } from './drafts'
import { ChoiceCards } from './ChoiceCards'
import { StampRoute } from './StampRoute'
import { Suspense, lazy } from 'react'
import { useI18n } from '../i18n'

/** The Markdown renderer is large and only needed once a document body is opened. */
const Markdown = lazy(() => import('../markdown/Markdown').then((module) => ({ default: module.Markdown })))

export interface PaperProps {
  item: DeskItem
  orgLabel: string
  viewerLabel: string
  draft: Draft
  /** When the viewer's seal was pressed on this item, if it was. */
  sealedAt: string | null
  isSending: boolean
  onChange: (change: Partial<Draft>) => void
  onSend: () => void
}

function Thread({ item }: { item: DeskItem }) {
  const { t, fmt } = useI18n()
  const messages = item.request.thread ?? []
  if (messages.length === 0) return null
  return (
    <div className="doc-thread">
      {messages.map((message) => (
        <div key={`${message.at}-${message.from}`} className="doc-msg">
          <b>{t.desk.paper.thread(personOf(item.view, message.from)?.label ?? message.from, item.request.options.find((option) => option.key === message.choice)?.text ?? message.choice ?? null)}</b>{' '}
          <span className="muted">{fmt.dateTime(message.at)}</span>
          {message.text === '' ? null : <p>{message.text}</p>}
        </div>
      ))}
    </div>
  )
}

function DecideForm({ item, draft, isSending, onChange, onSend }: Pick<PaperProps, 'item' | 'draft' | 'isSending' | 'onChange' | 'onSend'>) {
  const { t } = useI18n()
  const { options, kind } = item.request
  const verb = t.desk.paper.verbs[kind]
  const chosen = options.find((option) => option.key === draft.choice)
  return (
    <div className="doc-decide">
      <ChoiceCards name={`choice-${item.key}`} options={options} choice={draft.choice} isDisabled={isSending} onChoose={(choice) => onChange({ choice })} />
      <textarea
        className="answer-note"
        aria-label={t.desk.paper.noteLabel}
        placeholder={options.length > 0 ? t.desk.paper.notePlaceholderWithOptions : t.desk.paper.notePlaceholderOpen}
        value={draft.text}
        disabled={isSending}
        onChange={(event) => onChange({ text: event.target.value })}
      />
      <div className="doc-actions">
        <button className="btn primary big" type="button" disabled={isSending} onClick={onSend}>
          {isSending ? t.desk.paper.sending : chosen === undefined ? t.desk.paper.send(verb) : t.desk.paper.sendWith(verb, chosen.text)}
        </button>
        <span className="hint">
          <span className="kbd">⌘</span> <span className="kbd">Enter</span> {t.desk.paper.shortcutSend}
          {options.length > 0 ? ` · ${t.desk.paper.shortcutPick}` : ''}
        </span>
      </div>
    </div>
  )
}

/** One open document: head with the stamp boxes, context, thread, the form to decide, and the body. */
export function Paper(props: PaperProps) {
  const { item, orgLabel, viewerLabel, sealedAt } = props
  const { t, fmt } = useI18n()
  const { request } = item
  const deciderLabel = item.isMine ? viewerLabel : (item.decider?.label ?? request.to ?? '?')
  const stoppedNames = blockedPeople(item).map((person) => person.label).join(', ')
  const titleId = `doc-title-${item.key}`

  return (
    <article className={`paper${sealedAt === null ? '' : ' is-sealed'}`} aria-labelledby={titleId}>
      <header className="doc-head">
        <div>
          <p className="doc-type">{t.desk.paper.types[request.kind]}</p>
          <h2 className="doc-title" id={titleId}>{request.title}</h2>
          <p className="doc-meta">
            <span>{t.desk.paper.team(orgLabel, item.view.label)}</span>
            {stoppedNames === '' ? null : <span>{t.desk.paper.stoppedBy(stoppedNames)}</span>}
          </p>
        </div>
        <StampRoute drafter={item.drafter} deciderLabel={deciderLabel} step={request.kind === 'proposal' ? 'approved' : 'answered'} draftedAt={request.createdAt} draftedBy={request.from} sealedAt={sealedAt} />
      </header>
      {request.context === '' ? null : <p className="doc-context">{request.context}</p>}
      <Thread item={item} />
      {sealedAt !== null ? (
        <p className="doc-done" role="status">{t.desk.paper.sealedNote(deciderLabel, t.desk.paper.verbsDone[request.kind])}</p>
      ) : canDecide(item) ? (
        <DecideForm {...props} />
      ) : (
        <p className="doc-done">{t.desk.paper.waitingOn(deciderLabel)}</p>
      )}
      {request.document === undefined ? null : (
        <details className="doc-body">
          <summary>{t.desk.paper.readDocument(fmt.integer(request.document.length))}</summary>
          <Suspense fallback={<p className="muted">{t.app.loading}</p>}>
            <Markdown source={request.document} />
          </Suspense>
        </details>
      )}
    </article>
  )
}
