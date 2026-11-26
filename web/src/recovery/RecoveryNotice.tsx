import type { Me } from '../api/clientApi'
import { useI18n } from '../i18n'
import { isId, recoveriesOf, type Recovery } from './provisional'
import './recovery.css'

/** What the owner takes from `agency root-cancel-recovery`: the org's id, since the command asks for the id and not the label. */
const cancelCommand = (orgId: string): string => `agency root-cancel-recovery --org ${orgId}`
const COMMAND_NAME = 'agency root-cancel-recovery'

/**
 * A started recovery of an org that waits, at the top of the desk. The root's owner is told to cancel it, with the command; a
 * member of an org whose root key is on another node is only told that it is under way, and that its owner can cancel it. It has no way to be closed: it is there while the node says
 * the recovery waits, and goes when the node stops saying so (it was cancelled, or it took effect). Nothing is drawn for an
 * answer that has not come, which is why it takes the viewer once it is read.
 */
export function RecoveryNotice({ me }: { me: Me }) {
  const { t, fmt } = useI18n()
  const { recovery } = t.desk
  const headline = (one: Recovery): string => {
    const at = one.takesEffectAt === undefined ? undefined : fmt.dateTime(one.takesEffectAt)
    if (one.isRoot) return at === undefined ? recovery.started(one.org.label) : recovery.startedTakingEffect(one.org.label, at)
    return at === undefined ? recovery.underway(one.org.label) : recovery.underwayTakingEffect(one.org.label, at)
  }
  return (
    <>
      {recoveriesOf(me).map((one) => (
        <section key={one.org.id} className="recovery-notice" role={one.isRoot ? 'alert' : 'status'} aria-label={recovery.label}>
          <span className="recovery-mark" aria-hidden="true">{'\u26A0\uFE0E'}</span>
          <div className="recovery-body">
            <p>
              <strong>{headline(one)}</strong>
            </p>
            {one.newRootKey === undefined ? null : <p>{recovery.newRootKey(one.newRootKey)}</p>}
            {one.replacesRecoveryKey ? <p>{recovery.replacesRecoveryKey}</p> : null}
            {one.isRoot ? (
              <p>
                {isId(one.org.id) ? (
                  <>
                    {recovery.ifNotYou} <code>{cancelCommand(one.org.id)}</code>
                  </>
                ) : (
                  // An id the node would not have made is never put into a command to paste into a terminal: only the command's name is shown.
                  <>
                    {recovery.cancelByName[0]}
                    <code>{COMMAND_NAME}</code>
                    {recovery.cancelByName[1]}
                  </>
                )}
              </p>
            ) : (
              <p>{recovery.ownerCanCancel}</p>
            )}
          </div>
        </section>
      ))}
    </>
  )
}
