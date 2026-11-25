import type { DecisionAnswer, Envelope, Signed } from '../api/clientApi'
import { toHex } from './bytes'
import type { DeviceKey } from './deviceKey'
import { signDocument } from './signing'
import { timestampOf } from './time'

/** 16 random bytes as 32 lowercase hex characters, the shape of an envelope id. */
export const newEnvelopeId = (): string => toHex(crypto.getRandomValues(new Uint8Array(16)))

interface EnvelopeInput {
  answer: DecisionAnswer
  /** The deciding person's account id. */
  account: string
  id: string
  at: Date
}

/** The decision as the protocol's envelope: what is signed, without the notes only the log keeps. */
export function buildEnvelope({ answer, account, id, at }: EnvelopeInput): Envelope {
  return {
    id,
    kind: answer.answerWith.kind,
    org: answer.org,
    team: answer.team,
    target: answer.answerWith.target,
    payload: { text: answer.text, ...(answer.choice === undefined ? {} : { choice: answer.choice }) },
    account,
    at: timestampOf(at),
  }
}

/** The envelope signed by this device, named by `deviceId` as its signer. */
export const signEnvelope = (key: DeviceKey, deviceId: string, envelope: Envelope): Promise<Signed<Envelope>> => signDocument(key, 'envelope', envelope, deviceId)
