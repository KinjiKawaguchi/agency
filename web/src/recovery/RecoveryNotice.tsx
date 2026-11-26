import type { Me } from '../api/clientApi'
import { useI18n } from '../i18n'
import { recoveriesOf } from './provisional'
import './recovery.css'

/** What the owner takes from `agency root-cancel-recovery`: the org's id, since the command asks for the id and not the label. */
const cancelCommand = (orgId: string): string => `agency root-cancel-recovery --org ${orgId}`

/**
 * A started recovery of an org that waits, at the top of the desk. It has no way to be closed: it is there while the node says
 * the recovery waits, and goes when the node stops saying so (it was cancelled, or it took effect). Nothing is drawn for an
 * answer that has not come, which is why it takes the viewer once it is read.
 */
export function RecoveryNotice({ me }: { me: Me }) {
  const { t, fmt } = useI18n()
  const { recovery } = t.desk
  return (
    <>
      {recoveriesOf(me).map((one) => (
        <section key={one.org.id} className="recovery-notice" role="alert" aria-label={recovery.label}>
          <span className="recovery-mark" aria-hidden="true">{'\u26A0\uFE0E'}</span>
          <div className="recovery-body">
            <p>
              <strong>{one.takesEffectAt === undefined ? recovery.started(one.org.label) : recovery.startedTakingEffect(one.org.label, fmt.dateTime(one.takesEffectAt))}</strong>
            </p>
            {one.newRootKey === undefined ? null : <p>{recovery.newRootKey(one.newRootKey)}</p>}
            {one.replacesRecoveryKey ? <p>{recovery.replacesRecoveryKey}</p> : null}
            <p>
              {recovery.ifNotYou} <code>{cancelCommand(one.org.id)}</code>
            </p>
          </div>
        </section>
      ))}
    </>
  )
}
