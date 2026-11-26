// Provisional: the shapes of the claim link as designed on agency#196 and #201, until AgencyCore's types are generated into
// `bindings/`. Nothing else in the app writes these shapes down: when the generated types land, this file is replaced by
// imports of them, and the rest of `src/claim/` does not change.

/** What this browser sends to take a seat: the code of the link and the public key it just made. */
export interface ClaimInput {
  code: string
  publicKey: string
  label: string
}

/** Where the claim stands: waiting for the owner to compare the pairing code and confirm, or done. */
export interface ClaimState {
  state: 'waiting' | 'in'
  /** The seat's label, as the org writes it. */
  seat: string
  /** The org's label. */
  org: string
  /** The pairing code of the key this browser sent, for the owner to compare. */
  pairingCode: string
}

/** Why a claim is refused, from the node's status: unknown or used (404), expired (410), another address signed in (403), the seat or the key already taken (409). */
export type ClaimFailure = 'unknown' | 'expired' | 'other-email' | 'taken'
