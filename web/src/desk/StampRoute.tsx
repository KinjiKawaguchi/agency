import type { PersonStatus } from '../api/teamview'
import { useI18n } from '../i18n'
import { initials } from '../people/identity'

interface StampRouteProps {
  drafter: PersonStatus | undefined
  deciderLabel: string
  /** `approved` for proposals, `answered` for everything else. */
  step: 'approved' | 'answered'
  draftedAt: string
  /** Shown when the drafter is not a person on the team, such as the board. */
  draftedBy: string
  /** When the decider's stamp was pressed, if it was. */
  sealedAt: string | null
}

/** The drafted → approved boxes at the head of a document. Pressing send puts the decider's seal in their box. */
export function StampRoute({ drafter, deciderLabel, step, draftedAt, draftedBy, sealedAt }: StampRouteProps) {
  const { t, fmt } = useI18n()
  const drafterLabel = drafter?.label ?? draftedBy
  const stepLabel = t.desk.paper.stamps[step]
  return (
    <div className="stamps" role="group" aria-label={t.desk.paper.stamps.label(drafterLabel, stepLabel, deciderLabel)}>
      <div className="stamp-box">
        <span className="stamp-role">{t.desk.paper.stamps.drafted}</span>
        <span className="seal drafter">
          <b>{initials(drafterLabel)}</b>
        </span>
        <small className="stamp-time">{fmt.clock(draftedAt)}</small>
      </div>
      <div className={`stamp-box${sealedAt === null ? ' open' : ''}`}>
        <span className="stamp-role">{stepLabel}</span>
        {sealedAt === null ? (
          <span className="stamp-empty">{deciderLabel}</span>
        ) : (
          <>
            <span className="seal you just">
              <b>{initials(deciderLabel)}</b>
            </span>
            <small className="stamp-time">{fmt.clock(sealedAt)}</small>
          </>
        )}
      </div>
    </div>
  )
}
