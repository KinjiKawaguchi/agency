import type { Signed } from '../../../bindings/Signed'
import { toBase64 } from './bytes'
import { canonicalJson, type Json } from './canonical'

/** What a signature is for. The kind is part of the signed text, so a signature for one cannot serve another. */
export type SignedKind = 'envelope' | 'org' | 'join' | 'read' | 'device'

const encoder = new TextEncoder()

/** The bytes that are signed: a kind line, then the body's canonical JSON. */
export const signedText = (kind: SignedKind, body: Json): Uint8Array<ArrayBuffer> => encoder.encode(`agency/v1/${kind}\n${canonicalJson(body)}`)

/** Something that signs bytes with an Ed25519 key and answers the signature in base64. */
export interface Signer {
  sign: (message: Uint8Array<ArrayBuffer>) => Promise<string>
}

/** A signed document: the body, the id of the signing device, and the signature. */
export async function signDocument<T extends Json>(signer: Signer, kind: SignedKind, body: T, signerId: string): Promise<Signed<T>> {
  return { body, signer: signerId, signature: await signer.sign(signedText(kind, body)) }
}

export const signatureToBase64 = (signature: ArrayBuffer): string => toBase64(new Uint8Array(signature))
