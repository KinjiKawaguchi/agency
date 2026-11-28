import type { OrgParty } from '../api/clientApi'
import { useI18n } from '../i18n'

/** The seats no one has taken, or the external parties: what to ask each about and who answers for them. */
export function PartyList({ parties, labels, empty }: { parties: readonly OrgParty[]; labels: ReadonlyMap<string, string>; empty: string }) {
  const { t } = useI18n()
  const text = t.members
  if (parties.length === 0) return <p className="muted">{empty}</p>
  return (
    <ul className="mb-list">
      {parties.map((party) => (
        <li key={party.id} className="mb-member">
          <p className="mb-name">
            <b>{party.label}</b>
            {party.department === undefined ? null : <span className="muted"> {text.department(party.department)}</span>}
          </p>
          <p className="muted">{party.askAbout.length === 0 ? text.noTopics : text.askAbout(party.askAbout.join(', '))}</p>
          <p className="muted">{party.answeredBy.length === 0 ? text.ownerAnswers : text.answeredBy(party.answeredBy.map((id) => labels.get(id) ?? id).join(', '))}</p>
        </li>
      ))}
    </ul>
  )
}
