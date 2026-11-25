/** Standard base64 with `=` padding, as every key and signature in the protocol is written. */
export function toBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
}

export function fromBase64(text: string): Uint8Array<ArrayBuffer> {
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(text) || text.length % 4 !== 0) throw new RangeError('That is not base64.')
  return Uint8Array.from(atob(text), (character) => character.charCodeAt(0))
}

export const toHex = (bytes: Uint8Array): string => [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('')
