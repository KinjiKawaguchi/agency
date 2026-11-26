// Canonical JSON, as docs/protocol.md defines it and `agency-proto` writes it: keys sorted by their UTF-8
// bytes, no whitespace, strings escaped as standard JSON, integers only, and object fields whose value
// is null left out. Signer and checker must produce the same bytes, so this follows the Rust definition.

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json | undefined }

const encoder = new TextEncoder()

function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const shared = Math.min(a.length, b.length)
  for (let index = 0; index < shared; index += 1) {
    const difference = (a[index] ?? 0 /* stand-in-ok(index): bytes compared by position */) - (b[index] ?? 0)
    if (difference !== 0) return difference
  }
  return a.length - b.length
}

function write(value: Json | undefined): string {
  if (value === null || value === undefined) return 'null'
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) throw new RangeError(`Only integers can be signed, not ${value}.`)
    return String(value)
  }
  if (typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(write).join(',')}]`
  const entries = Object.entries(value)
    .filter((entry): entry is [string, Json] => entry[1] !== null && entry[1] !== undefined)
    .map(([key, one]) => ({ key, bytes: encoder.encode(key), one }))
    .toSorted((a, b) => compareBytes(a.bytes, b.bytes))
  return `{${entries.map(({ key, one }) => `${JSON.stringify(key)}:${write(one)}`).join(',')}}`
}

export const canonicalJson = (value: Json): string => write(value)
