import type { OrgMember } from '../api/clientApi'
import { useI18n } from '../i18n'

/** One member of the org: who they are, how they are listed, and the devices that sign for them. */
export function MemberCard({ member }: { member: OrgMember }) {
  const { t, fmt } = useI18n()
  const text = t.members
  return (
    <li className="mb-member">
      <p className="mb-name">
        <b>{member.label}</b> <span className="muted">{text.roles[member.role]}</span>
      </p>
      <p className="muted mb-facts">
        {[member.department === undefined ? null : text.department(member.department), member.isHosted ? text.hosted : null, member.hasSignIn === undefined ? null : member.hasSignIn ? text.hostedSignIn.set : text.hostedSignIn.unset]
          .filter((fact): fact is string => fact !== null)
          .join(' · ')}
      </p>
      {member.devices.length === 0 ? (
        <p className="muted">{text.noDevices}</p>
      ) : (
        <ul className="mb-devices">
          {member.devices.map((device) => (
            <li key={device.id}>{text.device(device.label, text.keyKinds[device.keyKind], fmt.relative(device.addedAt, t.time.justNow))}</li>
          ))}
        </ul>
      )}
    </li>
  )
}
