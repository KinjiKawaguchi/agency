import type { DecisionAnswer } from '../api/clientApi'
import type { ApiClient } from '../api/client'
import { deviceKeyOf, generateKeyPair, type DeviceKey, type KeyStore } from '../device/deviceKey'
import { buildEnvelope, newEnvelopeId, signEnvelope } from '../device/envelope'
import { deviceIdOf } from '../device/fingerprint'
import { createMockNode } from '../mock/node'

/** One device key for the whole test file: making a key is cheap, but each test should not have to. */
const pair = await generateKeyPair()
export const TEST_DEVICE: DeviceKey = await deviceKeyOf(pair)
export const TEST_DEVICE_ID = deviceIdOf(TEST_DEVICE.pairingCode)

/** A store that already holds the test device's key, as a browser that was used before. */
export function testKeyStore(): KeyStore {
  return { load: () => Promise.resolve(pair), save: () => Promise.resolve() }
}

/** A mock node whose account already has the test device, so what a test does as the owner is accepted. */
export const createTestNode = (options: NonNullable<Parameters<typeof createMockNode>[0]> = {}): ReturnType<typeof createMockNode> =>
  createMockNode({ ...options, devices: [{ id: TEST_DEVICE_ID, label: 'Browser', publicKey: TEST_DEVICE.publicKey }, ...(options.devices ?? [])] })

/** Decides as the test device would, straight at the node, for a test that needs decisions already made. */
export async function decideAsTestDevice(api: ApiClient, answer: DecisionAnswer, account = 'alice'): Promise<void> {
  const envelope = buildEnvelope({ answer, account, id: newEnvelopeId(), at: new Date() })
  await api.decideSigned(answer.org, answer.team, { envelope: await signEnvelope(TEST_DEVICE, TEST_DEVICE_ID, envelope), title: answer.title })
}
