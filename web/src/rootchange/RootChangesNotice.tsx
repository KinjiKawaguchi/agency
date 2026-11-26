import type { Me } from '../api/clientApi'
import { useI18n } from '../i18n'
import type { Messages } from '../i18n/en'
import { rootChangesOf, type RootChangeRef } from './provisional'
import './rootchange.css'

/** What a change does, in the page's words from its kind and fields; the node's English `what` for a kind that is not known or has lost a field, and a plain line when it has none either. */
function describe(change: RootChangeRef, text: Messages['desk']['rootChanges']): string {
  const { kind, member, label, device } = change
  if (kind === 'addMember' && member !== undefined && label !== undefined) return text.addMember(label, member)
  if (kind === 'addDevice' && member !== undefined && label !== undefined) return text.addDevice(label, member)
  if (kind === 'removeDevice' && member !== undefined && device !== undefined) return text.removeDevice(device, member)
  return change.what ?? text.unknownChange
}

/**
 * The changes others asked the root key to sign, which wait for the owner (`Me.rootChanges`): one item on the desk with how many
 * they are, what each changes, and the command that signs them together. Nothing is drawn for an answer
 * that has not come, and nothing when none wait: the node gives the list to the owner only.
 */
export function RootChangesNotice({ me }: { me: Me }) {
  const { t, fmt } = useI18n()
  const text = t.desk.rootChanges
  const changes = rootChangesOf(me)
  if (changes.length === 0) return null
  const labelOf = (id: string): string => me.orgs.find((org) => org.id === id)?.label ?? id
  return (
    <section className="rootchange-notice" role="status" aria-label={text.label}>
      <p>
        <strong>{text.title(changes.length)}</strong>
      </p>
      <ul>
        {changes.map((change) => (
          <li key={change.id}>{text.item(describe(change, text), labelOf(change.org), fmt.relative(change.receivedAt, t.time.justNow))}</li>
        ))}
      </ul>
      <p>
        {text.command[0]}
        <code>agency root-sign</code>
        {text.command[1]}
      </p>
    </section>
  )
}
