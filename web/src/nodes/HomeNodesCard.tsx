import { useState, type FormEvent } from 'react'
import { useI18n } from '../i18n'
import { isNativeApp, parseNodeAddress, withNode, withoutNode, withSelfLabel, MAX_NODE_LABEL } from './homeNodes'
import { useHomeNodes } from './useHomeNodes'

const REASONS = ['empty', 'not-an-address', 'needs-https', 'has-credentials'] as const

/**
 * The other home nodes of the owner's accounts, as links. A home node is an origin: this page never reads from
 * another one, so a link opens that node's own page, with its own sign-in and its own device key.
 * In the iPhone app the app's own switcher does this, so nothing is shown.
 */
export function HomeNodesCard() {
  const { t } = useI18n()
  const { nodes, change } = useHomeNodes()
  const [label, setLabel] = useState('')
  const [address, setAddress] = useState('')
  const [problem, setProblem] = useState<(typeof REASONS)[number] | null>(null)
  if (isNativeApp()) return null
  const text = t.devices.homeNodes
  const selfOrigin = window.location.origin

  const add = (event: FormEvent): void => {
    event.preventDefault()
    const parsed = parseNodeAddress(address)
    if (!parsed.ok) return setProblem(parsed.reason)
    change((current) => withNode(current, label, parsed.origin, selfOrigin))
    setLabel('')
    setAddress('')
    setProblem(null)
  }

  return (
    <section className="device-card" aria-labelledby="home-nodes">
      <h2 className="sub-head" id="home-nodes">{text.title}</h2>
      <p className="muted">{text.intro}</p>
      <label className="device-field">
        <span>{text.thisNode}</span>
        <input type="text" value={nodes.self ?? ''} placeholder={window.location.host} maxLength={MAX_NODE_LABEL} onChange={(event) => change((current) => withSelfLabel(current, event.target.value))} />
      </label>
      <p className="muted">{text.thisHost(window.location.host)}</p>
      {nodes.others.length === 0 ? (
        <p className="muted">{text.none}</p>
      ) : (
        <ul className="device-list node-list">
          {nodes.others.map((one) => (
            <li key={one.origin}>
              <a href={one.origin} rel="noopener noreferrer">{one.label}</a>
              <span className="muted">{new URL(one.origin).host}</span>
              <button className="btn small" type="button" onClick={() => change((current) => withoutNode(current, one.origin))} aria-label={text.remove(one.label)}>
                {text.removeShort}
              </button>
            </li>
          ))}
        </ul>
      )}
      <form onSubmit={add} noValidate>
        <label className="device-field">
          <span>{text.name}</span>
          <input type="text" value={label} maxLength={MAX_NODE_LABEL} onChange={(event) => setLabel(event.target.value)} />
        </label>
        <label className="device-field">
          <span>{text.address}</span>
          <input type="text" inputMode="url" autoCapitalize="none" autoCorrect="off" spellCheck={false} value={address} placeholder="https://" onChange={(event) => setAddress(event.target.value)} />
        </label>
        <button className="btn primary" type="submit">{text.add}</button>
        {problem === null ? null : <p className="device-invalid" role="alert">{text.problems[problem]}</p>}
      </form>
    </section>
  )
}
