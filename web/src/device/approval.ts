import type { DeviceList } from '../api/clientApi'
import { orgsLacking } from './enrollment'

/** Who can approve a device that waits to be added, org by org. */
export interface Approval {
  /** Orgs in which the account already has a device: one of those approves it. */
  deviceOrgs: readonly string[]
  /** The labels of those devices, each once, in the order the node lists them. */
  deviceLabels: readonly string[]
  /** Orgs in which the account has no device yet: only the node's own machine can approve the first one. */
  nodeOrgs: readonly string[]
}

/** How the device with this key gets approved: from a device the account has in each org that has one, on the node's machine in each that has none (docs/protocol.md, "Devices"). */
export function approvalOf(list: DeviceList, publicKey: string): Approval {
  const lacking = new Set(orgsLacking(list, publicKey))
  const held = list.orgs.filter((org) => lacking.has(org.org))
  const withDevices = held.filter((org) => org.devices.length > 0)
  return {
    deviceOrgs: withDevices.map((org) => org.org),
    deviceLabels: [...new Set(withDevices.flatMap((org) => org.devices.map((device) => device.label)))],
    nodeOrgs: held.filter((org) => org.devices.length === 0).map((org) => org.org),
  }
}

/** The longest command the page prints. */
const MAX_COMMAND = 500

/**
 * The command that approves a device on the node's machine, as the node wrote it for itself: the full path of the
 * running binary, its data directory when that is not the default, and `device-add`. The node sends it only to its
 * owner, so it may be missing. It is shown as text, never run.
 */
export function nodeCommandOf(list: DeviceList): string | undefined {
  const node = (list as { node?: unknown }).node
  const command = typeof node === 'object' && node !== null ? (node as { command?: unknown }).command : undefined
  // eslint-disable-next-line no-control-regex
  return typeof command === 'string' && command.trim() !== '' && command.length <= MAX_COMMAND && !/[\u0000-\u001f]/.test(command) ? command.trim() : undefined
}
