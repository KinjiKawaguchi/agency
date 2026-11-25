import type { SnapshotSource } from '../api/clientApi'
import { useI18n } from '../i18n'

/** Warns when a team's data is a copy that could not be refreshed, so the viewer knows how old it is. */
export function ReplicaNotice({ source }: { source: SnapshotSource }) {
  const { t, fmt } = useI18n()
  if (source.kind !== 'replica' || source.error === undefined) return null
  return (
    <p className="replica-notice" role="status">
      {source.fetchedAt === undefined ? t.app.replica.neverRead(source.error) : t.app.replica.staleCopy(fmt.dateTime(source.fetchedAt), source.error)}
    </p>
  )
}
