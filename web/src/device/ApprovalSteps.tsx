import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import type { DeviceList } from '../api/clientApi'
import { meQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { approvalOf, nodeCommandOf } from './approval'

/** What to do next for a device that waits to be added: from which of the account's devices, or on the node's machine with which command. */
export function ApprovalSteps({ list, publicKey }: { list: DeviceList; publicKey: string }) {
  const { t } = useI18n()
  const api = useApi()
  const me = useQuery(meQuery(api))
  const approval = approvalOf(list, publicKey)
  const command = nodeCommandOf(list)
  const text = t.devices.approval
  const labelOf = (org: string): string => me.data?.orgs.find((one) => one.id === org)?.label ?? org
  // Orgs are named only when the two ways differ between them.
  const isMixed = approval.deviceOrgs.length > 0 && approval.nodeOrgs.length > 0
  const where = (orgs: readonly string[]): string => (isMixed ? `${text.inOrgs(orgs.map(labelOf).join(', '))} ` : '')
  return (
    <div className="device-approval">
      <h3 className="sub-head">{text.title}</h3>
      <p className="device-why">{text.why}</p>
      {approval.deviceOrgs.length === 0 ? null : (
        <p className="device-way">
          {where(approval.deviceOrgs)}
          {text.fromDevices(approval.deviceLabels.join(', '))}
        </p>
      )}
      {approval.nodeOrgs.length === 0 ? null : (
        <div className="device-way">
          <p>
            {where(approval.nodeOrgs)}
            {text.onNode}
          </p>
          <code className="device-command">{command ?? text.fallbackCommand}</code>
          <p className="muted">{command === undefined ? text.fallbackNote : text.onNodeNote}</p>
        </div>
      )}
      {approval.deviceOrgs.length === 0 ? null : <p className="muted">{text.notTerminal}</p>}
    </div>
  )
}
