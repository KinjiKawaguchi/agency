import type { Objective } from '../api/metricsApi'
import { useI18n } from '../i18n'
import { groupName } from './groupName'
import { useBoundText } from './useBound'

/** The objectives the operator set, and what the node says of each: it is the node that compares, so this only says it. */
export function Objectives({ objectives }: { objectives: readonly Objective[] }) {
  const { t } = useI18n()
  const text = t.metrics.objectives
  const { bound } = useBoundText()
  if (objectives.length === 0) return null
  return (
    <div className="card mt-card" aria-labelledby="mt-objectives">
      <h3 className="sub-head" id="mt-objectives">{text.title}</h3>
      <p className="muted">{text.lead}</p>
      <ul className="mt-objectives">
        {objectives.map((one) => (
          <li key={one.group} className={`mt-objective is-${one.state}`}>
            <span>{text.target(groupName(t.metrics.groups.names, one.group), bound({ kind: 'atMost', ms: one.p95Ms }))}</span>
            <strong>{text.states[one.state]}</strong>
            {one.measuredP95Ms === undefined ? null : <span className="muted">{text.measured(bound({ kind: 'atMost', ms: one.measuredP95Ms }))}</span>}
          </li>
        ))}
      </ul>
    </div>
  )
}
