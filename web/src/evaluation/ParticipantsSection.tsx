import type { ParticipantEvaluation } from '../api/evaluationApi'
import { useI18n } from '../i18n'
import { printable } from '../shared/printable'
import { formatDuration } from './duration'

interface Props {
  participants: readonly ParticipantEvaluation[]
  /** The merged pull requests the runtime named no author for; left out of the answer when the node has none to say. */
  unattributedMerged: number | undefined
}

const tokensOf = (participant: ParticipantEvaluation): number => participant.tokens.input + participant.tokens.output + participant.tokens.cacheRead + participant.tokens.cacheWrite

/** The three spans of a participant's alive time as shares of their sum; nothing when there was none. */
function sharesOf(participant: ParticipantEvaluation): Array<{ key: 'working' | 'waiting' | 'idle'; seconds: number; share: number }> | undefined {
  const total = participant.busySeconds + participant.waitingSeconds + participant.idleSeconds
  if (total <= 0) return undefined
  return [
    { key: 'working', seconds: participant.busySeconds, share: participant.busySeconds / total },
    { key: 'waiting', seconds: participant.waitingSeconds, share: participant.waitingSeconds / total },
    { key: 'idle', seconds: participant.idleSeconds, share: participant.idleSeconds / total },
  ]
}

function Row({ participant }: { participant: ParticipantEvaluation }) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.participants
  const duration = (seconds: number): string => formatDuration(seconds, t.evaluation.units)
  const spans = sharesOf(participant)
  return (
    <li className="ev-person">
      <h4>{printable(participant.name)}</h4>
      {spans === undefined ? (
        <p className="muted">{text.noSession}</p>
      ) : (
        <div className="ev-time">
          <div className="ev-timebar" role="img" aria-label={text.timeLabel(duration(participant.busySeconds), duration(participant.waitingSeconds), duration(participant.idleSeconds))}>
            {spans.map((span) => (span.share > 0 ? <span key={span.key} className={`ev-span is-${span.key}`} style={{ flex: span.share }} /> : null))}
          </div>
          <ul className="legend ev-timelegend">
            {spans.map((span) => (
              <li key={span.key}>
                <i className={`ev-swatch is-${span.key}`} />
                {text[span.key]} {duration(span.seconds)}
              </li>
            ))}
          </ul>
        </div>
      )}
      <dl className="ev-facts">
        <div>
          <dt>{text.cost}</dt>
          <dd>
            {fmt.money(participant.costUsd)} <span className="muted">{text.tokens(fmt.integer(tokensOf(participant)))}</span>
          </dd>
        </div>
        <div>
          <dt>{text.merged}</dt>
          <dd>{participant.merged}</dd>
        </div>
        <div>
          <dt>{text.perPr}</dt>
          <dd>{participant.costPerMergedPrUsd === undefined ? <span className="muted">{text.noMerged}</span> : fmt.money(participant.costPerMergedPrUsd)}</dd>
        </div>
      </dl>
    </li>
  )
}

/**
 * How each participant of a team spent the period, and what it cost: what the node gives only to the team's owner. The
 * screen shows it when it is there and says nothing when it is not (that is not an error, and not a hint of what is missing).
 */
export function ParticipantsSection({ participants, unattributedMerged }: Props) {
  const { t } = useI18n()
  const text = t.evaluation.participants
  // A period with no participant to tell of is not a card with a heading and no one in it.
  if (participants.length === 0) return null
  return (
    <div className="card ev-participants">
      <p className="card-head">
        <b>{text.title}</b> <span className="muted">{text.lead}</span>
      </p>
      <ul className="ev-people">
        {participants.map((participant) => (
          <Row key={participant.name /* shown-ok(key): the key of a row, never drawn */} participant={participant} />
        ))}
      </ul>
      <p className="muted ev-note">{text.timeNote}</p>
      {unattributedMerged === undefined || unattributedMerged === 0 ? null : <p className="muted ev-note">{text.unattributed(unattributedMerged)}</p>}
    </div>
  )
}
