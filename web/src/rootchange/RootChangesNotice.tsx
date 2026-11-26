import type { Me } from '../api/clientApi'
import { useI18n } from '../i18n'
import { rootChangesOf } from './provisional'
import './rootchange.css'

/**
 * The changes others asked the root key to sign, which wait for the owner (`Me.rootChanges`): one item on the desk with how many
 * they are, what each changes (in the node's words), and the command that signs them together. Nothing is drawn for an answer
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
          <li key={change.id}>{text.item(change.what, labelOf(change.org), fmt.relative(change.receivedAt, t.time.justNow))}</li>
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
