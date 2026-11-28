import type { HistoryEntry } from '../api/clientApi'
import { useI18n } from '../i18n'

/** One version of the org as the node says it: when it was signed and by which key, what the signature showed, whether it follows the one before, and its changes. */
export function VersionRow({ entry }: { entry: HistoryEntry }) {
  const { t, fmt } = useI18n()
  const text = t.history
  return (
    <li className="hs-version">
      <p className="hs-head">
        <b>{text.version(entry.version)}</b>
        {entry.at === undefined ? null : <span className="muted"> {text.signed(fmt.relative(entry.at, t.time.justNow))}</span>}
        {entry.signedBy === undefined ? null : <span className="muted"> {text.signedBy(entry.signedBy)}</span>}
      </p>
      {entry.isKept ? <KeptVersion entry={entry} /> : <p className="muted hs-gap">{text.notKept}</p>}
    </li>
  )
}

function KeptVersion({ entry }: { entry: HistoryEntry }) {
  const { t } = useI18n()
  const text = t.history
  return (
    <>
      {entry.signature === undefined ? null : <p className={`hs-signature ${entry.signature}`}>{text.signatures[entry.signature]}</p>}
      {entry.follows === false ? <p className="hs-fork">{text.fork}</p> : null}
      {entry.comparedWith === undefined ? null : <p className="muted">{text.comparedWith(entry.comparedWith)}</p>}
      {entry.changes.length === 0 ? (
        <p className="muted">{text.noChanges}</p>
      ) : (
        <ul className="hs-changes">
          {entry.changes.map((change, index) => (
            <li key={`${index}:${change}`}>{change}</li>
          ))}
        </ul>
      )}
    </>
  )
}
