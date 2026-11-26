import { useI18n } from '../i18n'
import { blockedTotal, epicSegments, isDone, sortEpics, type Epic, type SegmentKind } from './work'

const LEGEND: readonly SegmentKind[] = ['done', 'doing', 'blocked', 'rest']

function EpicRow({ epic }: { epic: Epic }) {
  const { t, fmt } = useI18n()
  const isFinished = isDone(epic)
  const blockedText = epic.blocked.map((entry) => `${t.people.blockedReasons[entry.reason]} ${entry.count}`).join(', ')
  return (
    <div className={`goal${isFinished ? ' is-done' : ''}`}>
      <div className="goal-top">
        <p className="goal-title">{epic.title}</p>
        {isFinished ? <span className="goal-done">{t.people.epicDone}</span> : null}
        <span className="goal-count">
          {epic.done} / {epic.total}
        </span>
      </div>
      <div className="goal-bar" role="img" aria-label={t.people.epicBar(epic.total, epic.done, epic.doing, blockedTotal(epic))}>
        {epicSegments(epic).map((segment) => (
          <span key={segment.kind} className={`seg ${segment.kind}`} style={{ flex: segment.count }} />
        ))}
      </div>
      <p className="goal-meta" title={epic.basis}>
        <span className="mono ref">{epic.ref}</span>
        <span>{isFinished ? (epic.doneAt === undefined ? t.people.epicDone : t.people.epicDoneOn(fmt.dateTime(epic.doneAt))) : epic.forecast === undefined ? epic.basis : t.people.epicFinish(fmt.dateTime(epic.forecast.at))}</span>
        {blockedText === '' ? null : <span className="work-stop">{t.people.epicBlockedBy(blockedText)}</span>}
      </p>
    </div>
  )
}

/** Epic forecasts: how far along each epic is and when it should finish; the epics finished lately come after, marked done, with no forecast. */
export function EpicList({ epics }: { epics: readonly Epic[] }) {
  const { t } = useI18n()
  return (
    <>
      <h3 className="sub-head">{t.people.epicsTitle}</h3>
      {epics.length === 0 ? (
        <p className="empty">{t.people.noEpics}</p>
      ) : (
        <div className="goals">
          <div className="goal-legend" aria-hidden="true">
            {LEGEND.map((kind) => (
              <span key={kind}>
                <i className={kind} />
                {t.people.legend[kind]}
              </span>
            ))}
          </div>
          {sortEpics(epics).map((epic) => (
            <EpicRow key={epic.ref} epic={epic} />
          ))}
        </div>
      )}
    </>
  )
}
