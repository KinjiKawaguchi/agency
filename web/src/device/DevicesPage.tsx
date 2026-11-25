import { useEffect, useState } from 'react'
import { useI18n } from '../i18n'
import { pairingCodeOf } from './fingerprint'
import { useDeviceKey } from './useDeviceKey'
import './device.css'

type Checked = { status: 'empty' } | { status: 'invalid' } | { status: 'valid'; code: string }

/** The pairing code of whatever public key was pasted, as it is worked out. */
function useCheckedKey(text: string): Checked {
  const [checked, setChecked] = useState<Checked>({ status: 'empty' })
  useEffect(() => {
    let isCurrent = true
    if (text.trim() === '') {
      setChecked({ status: 'empty' })
      return
    }
    pairingCodeOf(text).then(
      (code) => isCurrent && setChecked({ status: 'valid', code }),
      () => isCurrent && setChecked({ status: 'invalid' }),
    )
    return () => {
      isCurrent = false
    }
  }, [text])
  return checked
}

function ThisDevice() {
  const { t } = useI18n()
  const state = useDeviceKey()
  if (state.status === 'loading') return <p className="muted">{t.devices.loading}</p>
  if (state.status === 'unsupported') return <p className="empty" role="alert">{t.devices.unsupported}</p>
  if (state.status === 'failed') return <p className="empty" role="alert">{t.devices.failed(state.reason)}</p>
  return (
    <>
      <p className="device-lead">{t.devices.thisLead}</p>
      <p className="pairing-code" aria-label={t.devices.codeLabel}>{state.key.pairingCode}</p>
      <details className="device-key">
        <summary>{t.devices.showKey}</summary>
        <p className="muted">{t.devices.keyHint}</p>
        <code className="device-key-text" aria-label={t.devices.keyLabel}>{state.key.publicKey}</code>
      </details>
    </>
  )
}

function CheckAnother() {
  const { t } = useI18n()
  const [text, setText] = useState('')
  const checked = useCheckedKey(text)
  return (
    <>
      <p className="device-lead">{t.devices.checkLead}</p>
      <label className="device-field">
        <span>{t.devices.pasteLabel}</span>
        <textarea rows={2} value={text} spellCheck={false} placeholder={t.devices.pastePlaceholder} onChange={(event) => setText(event.target.value)} />
      </label>
      {checked.status === 'invalid' ? <p className="device-invalid" role="alert">{t.devices.invalidKey}</p> : null}
      {checked.status === 'valid' ? (
        <div className="device-compare" role="status">
          <p className="pairing-code" aria-label={t.devices.otherCodeLabel}>{checked.code}</p>
          <p>{t.devices.compare}</p>
        </div>
      ) : null}
    </>
  )
}

/** This device's pairing code, and a way to read the code of another device's key before adding it. */
export function DevicesPage() {
  const { t } = useI18n()
  return (
    <>
      <h1 className="page-title">{t.devices.title}</h1>
      <p className="muted">{t.devices.intro}</p>
      <section className="device-card">
        <h2 className="sub-head">{t.devices.thisDevice}</h2>
        <ThisDevice />
      </section>
      <section className="device-card">
        <h2 className="sub-head">{t.devices.addAnother}</h2>
        <CheckAnother />
      </section>
    </>
  )
}
