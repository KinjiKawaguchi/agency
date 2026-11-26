import { useEffect, useRef, useState } from 'react'
import { useApi } from '../api/context'
import { ApiError, type ApiClient } from '../api/client'
import { SignInExpiredError } from '../api/session'
import { useSettledDeviceKey, type DeviceKeyState } from '../device/useDeviceKey'
import { useI18n } from '../i18n'
import { forgetClaimCode, takeClaimCode } from './claimCode'
import type { ClaimFailure, ClaimState } from './provisional'

/** How often the page asks whether the owner has confirmed, while it waits (and the tab is in front). */
export const CLAIM_POLL_MS = 3000

export type ClaimProblem = ClaimFailure | 'mismatch' | 'signed-out' | 'unreachable' | 'failed'

export type ClaimView =
  | { step: 'loading' }
  | { step: 'no-code' }
  | { step: 'unsupported' }
  | { step: 'waiting'; claim: ClaimState }
  | { step: 'in'; claim: ClaimState }
  | { step: 'failed'; problem: ClaimProblem; message: string }

const STATUS: Record<number, ClaimFailure> = { 404: 'unknown', 410: 'expired', 403: 'other-email', 409: 'taken' }

/** What an error from the node means for the claim. A call that got no answer at all is the node being out of reach. */
function problemOf(error: unknown): { problem: ClaimProblem; message: string } {
  const message = error instanceof Error ? error.message : String(error)
  if (error instanceof SignInExpiredError) return { problem: 'signed-out', message }
  if (error instanceof ApiError) return { problem: STATUS[error.status] ?? 'failed', message }
  return { problem: 'unreachable', message }
}

/**
 * Where the claim stands, with the pairing code of **this browser's own key**, worked out here as the Devices page and the phone
 * do. The node's answer carries one too, and it is only checked against ours: if it differs, the node holds another key than
 * the one this browser made, and a code the owner compares would bind nothing. Then there is a refusal and no code.
 */
function stepOf(claim: ClaimState, ownCode: string): ClaimView {
  if (claim.pairingCode !== ownCode) return { step: 'failed', problem: 'mismatch', message: '' }
  return { step: claim.state === 'in' ? 'in' : 'waiting', claim: { ...claim, pairingCode: ownCode } }
}

/** A code that can be tried again after the person has put it right (another address) or the node is back; the others are spent. */
const isRetryable = (problem: ClaimProblem): boolean => problem === 'other-email' || problem === 'signed-out' || problem === 'unreachable'

/** What the claim needs of the page, read when it runs. */
interface Needs {
  current: { api: ApiClient; settledKey: () => Promise<DeviceKeyState>; label: string }
}

/** One try: takes the code, makes the key, claims the seat; or, with no code, asks where an earlier claim stands. Never rejects. */
async function claim({ current }: Needs): Promise<ClaimView> {
  const failed = (error: unknown): ClaimView => {
    const { problem, message } = problemOf(error)
    if (!isRetryable(problem)) forgetClaimCode()
    return { step: 'failed', problem, message }
  }
  const code = takeClaimCode()
  if (code === undefined) {
    // No link in the address: a claim that was made already (a reload, a return) is asked about, and nothing else is.
    const answered = await current.api.claimState().then(
      (claimed) => claimed,
      (error: unknown) => (error instanceof ApiError && error.status === 404 ? undefined : failed(error)),
    )
    if (answered === undefined) return { step: 'no-code' }
    if ('step' in answered) return answered
    // Where it stands is about the key this browser holds (the one it made when it claimed), so that is the one the code is worked out from.
    const held = await current.settledKey()
    return held.status === 'ready' ? stepOf(answered, held.key.pairingCode) : { step: 'failed', problem: 'mismatch', message: '' }
  }
  const key = await current.settledKey()
  if (key.status !== 'ready') return key.status === 'failed' ? { step: 'failed', problem: 'failed', message: key.reason } : { step: 'unsupported' }
  return current.api.redeemClaim({ code, publicKey: key.key.publicKey, label: current.label }).then((claimed) => {
    forgetClaimCode()
    return stepOf(claimed, key.key.pairingCode)
  }, failed)
}

/**
 * The claim of a seat by link, from the page the person lands on after signing in: takes the code (the address,
 * else this tab), makes this browser's key, claims the seat with both, then waits for the owner to confirm the pairing
 * code. A page read again with no code asks where the claim stands.
 */
export function useClaim(): { view: ClaimView; retry: () => void } {
  const api = useApi()
  const settledKey = useSettledDeviceKey()
  const { t } = useI18n()
  const [view, setView] = useState<ClaimView>({ step: 'loading' })
  const [attempt, setAttempt] = useState(0)
  // What the claim needs, as the page has it now: the page is drawn again while the claim is on its way (the key's state, the
  // language), and none of that may start a second claim, which the node would refuse as a link already used.
  const latest = useRef({ api, settledKey, label: t.devices.defaultLabel })
  latest.current = { api, settledKey, label: t.devices.defaultLabel }
  const started = useRef<{ attempt: number; outcome: Promise<ClaimView> } | null>(null)

  useEffect(() => {
    if (started.current?.attempt !== attempt) started.current = { attempt, outcome: claim(latest) }
    let isCurrent = true
    void started.current.outcome.then((next) => {
      if (isCurrent) setView(next)
    })
    return () => {
      isCurrent = false
    }
  }, [attempt])

  const isWaiting = view.step === 'waiting'
  const ownCode = view.step === 'waiting' ? view.claim.pairingCode : ''
  useEffect(() => {
    if (!isWaiting) return undefined
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'hidden') return
      void api.claimState().then(
        (claimed) => setView(stepOf(claimed, ownCode)),
        () => undefined,
      )
    }, CLAIM_POLL_MS)
    return () => window.clearInterval(timer)
  }, [api, isWaiting, ownCode])

  return { view, retry: () => setAttempt((n) => n + 1) }
}
