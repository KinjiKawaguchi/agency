import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import type { KpiInfo } from '../api/outcomesApi'
import { useI18n } from '../i18n'
import { useToast } from '../shared/Toasts'
import { formatKpiValue } from './format'
import { checkNumber, type Problem } from './numberInput'

interface Props {
  org: string
  kpi: KpiInfo
  /** The node's today: the days the node takes are counted from it. */
  today: string
}

/** What the owner enters: a number of the KPI for a day. The node decides; the checks here only say a mistake before it is sent. */
export function EnterNumber({ org, kpi, today }: Props) {
  const { t, locale } = useI18n()
  const text = t.outcomes.enter
  const api = useApi()
  const toast = useToast()
  const queryClient = useQueryClient()
  const [isOpen, setIsOpen] = useState(false)
  const [form, setForm] = useState({ date: today, value: '', note: '' })
  const [problem, setProblem] = useState<Problem | undefined>(undefined)
  const save = useMutation({
    mutationFn: (input: Parameters<typeof api.enterNumber>[1]) => api.enterNumber(org, input),
    onSuccess: (entry) => {
      toast(text.saved(formatKpiValue(entry.value, kpi, locale, t.evaluation.units), entry.date))
      setForm((now) => ({ ...now, value: '', note: '' }))
      void queryClient.invalidateQueries({ queryKey: ['outcomes', org] })
      void queryClient.invalidateQueries({ queryKey: ['readings', org] })
      void queryClient.invalidateQueries({ queryKey: ['number-history', org, kpi.id] })
    },
  })
  const failure = save.error === null ? undefined : save.error instanceof ApiError && save.error.status === 409 ? text.full : save.error instanceof ApiError && save.error.status === 403 ? text.forbidden : save.error instanceof ApiError && save.error.status < 500 ? text.refused(save.error.message) : failureText(t, save.error)
  const hint = kpi.unit === 'money' ? (kpi.currency === undefined ? text.hint.unknown : text.hint.money(kpi.currency)) : text.hint[kpi.unit]
  const submit = (): void => {
    const checked = checkNumber(form, kpi.id, today)
    setProblem(checked.ok ? undefined : checked.problem)
    if (checked.ok) save.mutate(checked.input)
  }
  if (!isOpen) {
    return (
      <button className="btn small" type="button" onClick={() => setIsOpen(true)}>
        {text.open}
      </button>
    )
  }
  return (
    <form
      className="oc-enter"
      aria-label={`${text.open}: ${kpi.label}`}
      onSubmit={(event) => {
        event.preventDefault()
        submit()
      }}
    >
      <label>
        <span>{text.day}</span>
        <input type="date" value={form.date} onChange={(event) => setForm({ ...form, date: event.target.value })} aria-invalid={problem === 'date'} />
      </label>
      <label>
        <span>{text.value}</span>
        <input type="text" inputMode="decimal" value={form.value} onChange={(event) => setForm({ ...form, value: event.target.value })} aria-invalid={problem === 'value'} aria-describedby={`oc-hint-${kpi.id}`} />
      </label>
      <p className="muted oc-hint" id={`oc-hint-${kpi.id}`}>{hint} {text.again}</p>
      <label>
        <span>{text.note}</span>
        <input type="text" value={form.note} onChange={(event) => setForm({ ...form, note: event.target.value })} aria-invalid={problem === 'note'} />
      </label>
      {problem === undefined ? null : <p role="alert" className="oc-problem">{text.problems[problem]}</p>}
      {failure === undefined ? null : <p role="alert" className="oc-problem">{failure}</p>}
      <div className="oc-enter-actions">
        <button className="btn small primary" type="submit" disabled={save.isPending}>
          {save.isPending ? text.saving : text.save}
        </button>
        <button className="btn small" type="button" onClick={() => {
            save.reset()
            setProblem(undefined)
            setIsOpen(false)
          }}>
          {text.close}
        </button>
      </div>
    </form>
  )
}
