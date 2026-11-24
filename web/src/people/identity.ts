const HUE_COUNT = 8

/** A stable hue number (1-8) from a name, so a person keeps their color as the team changes. */
export function hueOf(name: string): number {
  let hash = 0
  for (const char of name) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) >>> 0
  return (hash % HUE_COUNT) + 1
}

/** One or two characters for inside a face: Build1 → B1, TechLead → TL, Alice → Al, 花子 → 花. */
export function initials(label: string): string {
  const first = label.codePointAt(0)
  if (first === undefined) return ''
  if (first > 0x7f) return String.fromCodePoint(first)
  const numbered = /^([A-Za-z])[A-Za-z]*?(\d+)$/.exec(label)
  if (numbered) return `${(numbered[1] ?? '').toUpperCase()}${numbered[2] ?? ''}`
  const capitals = label.replace(/[^A-Z]/g, '')
  if (capitals.length >= 2) return capitals.slice(0, 2)
  return `${label.slice(0, 1).toUpperCase()}${label.slice(1, 2)}`
}
