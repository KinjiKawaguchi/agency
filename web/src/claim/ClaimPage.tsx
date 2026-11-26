import { Link } from '@tanstack/react-router'
import { useI18n } from '../i18n'
import { Skeleton } from '../shared/Skeleton'
import { useClaim, type ClaimProblem } from './useClaim'
import '../device/device.css'
import './claim.css'

/** Where Access ends the sign-in of this host: the same path on every host it protects. */
const SIGN_OUT_PATH = '/cdn-cgi/access/logout'

function Problem({ problem, message, onTryAgain }: { problem: ClaimProblem; message: string; onTryAgain: () => void }) {
  const { t } = useI18n()
  const text = t.claim.problems
  const sentence = problem === 'failed' ? text.failed(message) : text[problem]
  return (
    <>
      <p className="device-invalid" role="alert">{sentence}</p>
      <p className="claim-actions">
        {problem === 'other-email' ? <a className="btn primary" href={SIGN_OUT_PATH}>{t.claim.signOut}</a> : null}
        {problem === 'signed-out' ? <button className="btn primary" type="button" onClick={() => window.location.reload()}>{t.claim.reload}</button> : null}
        {problem === 'other-email' || problem === 'unreachable' ? <button className="btn" type="button" onClick={onTryAgain}>{t.claim.tryAgain}</button> : null}
      </p>
    </>
  )
}

/**
 * The page a claim link leads to once the person has signed in: it takes the seat with the key of this browser, shows
 * the pairing code for the owner to compare, and says when the person is in. It is outside the app's own bar: a person
 * who has not taken the seat has nothing of the app to open yet.
 */
export function ClaimPage() {
  const { t } = useI18n()
  const { view, retry } = useClaim()
  const text = t.claim
  return (
    <section className="claim device-card" aria-labelledby="claim-title">
      <p className="claim-brand"><span className="brand-seal" aria-hidden="true" /> {text.brand}</p>
      <h1 className="page-title" id="claim-title">{view.step === 'in' ? text.inTitle : view.step === 'waiting' ? text.waitingTitle : text.title}</h1>
      {view.step === 'loading' ? (
        <>
          <p className="muted" role="status">{text.working}</p>
          <Skeleton kind="line" />
        </>
      ) : null}
      {view.step === 'no-code' ? <p className="device-lead">{text.noCode}</p> : null}
      {view.step === 'unsupported' ? <p className="device-invalid" role="alert">{text.unsupported}</p> : null}
      {view.step === 'failed' ? <Problem problem={view.problem} message={view.message} onTryAgain={retry} /> : null}
      {view.step === 'waiting' ? (
        <>
          <p className="device-lead">{text.waitingLead(view.claim.seat, view.claim.org)}</p>
          <p className="pairing-code large" aria-label={text.codeLabel}>{view.claim.pairingCode}</p>
          <p className="device-compare-note">{text.compare}</p>
        </>
      ) : null}
      {view.step === 'in' ? (
        <>
          <p className="device-lead" role="status">{text.inLead(view.claim.seat, view.claim.org)}</p>
          <p className="claim-actions"><Link className="btn primary" to="/desk">{text.toDesk}</Link></p>
        </>
      ) : null}
    </section>
  )
}
