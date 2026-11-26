import type { Messages } from '../i18n/en'

/** The screen's name for a route group; a group a newer node names is shown by the node's own word. Looked up by own key, so `constructor` is not a name. */
export function groupName(names: Messages['metrics']['groups']['names'], group: string): string {
  return Object.hasOwn(names, group) ? (names[group] ?? group) : group
}
