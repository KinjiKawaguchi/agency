import type { OrgMembers } from '../api/clientApi'
import { useI18n } from '../i18n'
import { MemberCard } from './MemberCard'
import { PartyList } from './PartyList'

/** What the node says of one org's people: its members, the seats that are open, the external parties, and the changes that wait for the root key. */
export function OrgMembersView({ members }: { members: OrgMembers }) {
  const { t, fmt } = useI18n()
  const text = t.members
  const names = new Map(members.members.map((member) => [member.id, member.label]))
  const held = members.held ?? [] /* stand-in-ok(optional): the node leaves `held` out when nothing waits, in an answer that has come */
  return (
    <>
      <section className="mb-section">
        <h2 className="sub-head">{text.members}</h2>
        {members.members.length === 0 ? (
          <p className="muted">{text.noMembers}</p>
        ) : (
          <ul className="mb-list">
            {members.members.map((member) => (
              <MemberCard key={member.id} member={member} />
            ))}
          </ul>
        )}
      </section>
      <section className="mb-section">
        <h2 className="sub-head">{text.seats}</h2>
        <PartyList parties={members.seats} labels={names} empty={text.noSeats} />
      </section>
      <section className="mb-section">
        <h2 className="sub-head">{text.externals}</h2>
        <PartyList parties={members.externals} labels={names} empty={text.noExternals} />
      </section>
      <section className="mb-section">
        <h2 className="sub-head">{text.held}</h2>
        {held.length === 0 ? (
          <p className="muted">{text.noneHeld}</p>
        ) : (
          <>
            <p className="muted">{text.heldLead}</p>
            <ul className="mb-list">
              {held.map((change) => (
                <li key={change.id}>{text.heldAt(change.what, fmt.relative(change.receivedAt, t.time.justNow))}</li>
              ))}
            </ul>
          </>
        )}
      </section>
    </>
  )
}
