import { useI18n } from '../i18n'
import { ChartFrame } from './ChartFrame'
import { positionOf } from './series'
import { Tip } from './Tip'
import { useDays } from './useDays'

export interface Segment {
  key: string
  label: string
  color: string
  value: number
}

export interface ColumnData {
  date: string
  /** In stacking order, the first at the bottom. */
  segments: readonly Segment[]
}

interface ColumnsProps {
  columns: readonly ColumnData[]
  max: number
  format: (value: number) => string
  label: string
  /** How a day reads aloud and in its tip's total, from its segments' sum. */
  describe: (date: string, total: number) => string
  /** Whether the tip adds a total below its rows; true for a stack of several types. */
  hasTotal?: boolean
}

/** Columns by day, each one a stack of its segments. A single segment makes a plain column chart. */
export function Columns({ columns, max, format, label, describe, hasTotal = false }: ColumnsProps) {
  const { fmt } = useI18n()
  const days = useDays(columns.length)
  return (
    <ChartFrame max={max} format={format} dates={columns.map((column) => column.date)} label={label}>
      <div className="ev-cols" onKeyDown={days.onKeyDown}>
        {columns.map((column, index) => {
          const total = column.segments.reduce((sum, segment) => sum + segment.value, 0)
          const isSingle = column.segments.length === 1
          const rows = column.segments.filter((segment) => isSingle || segment.value > 0)
          return (
            <div key={column.date} className="ev-col" {...days.itemProps(index)} aria-label={describe(fmt.shortDate(column.date), total)}>
              <div className="ev-stack" style={{ height: `${Math.min(total / max, 1) * 100}%` }}>
                {column.segments.map((segment) => (segment.value > 0 ? <span key={segment.key} className="ev-slice" style={{ flex: segment.value, background: segment.color }} /> : null))}
              </div>
              <Tip
                title={fmt.shortDate(column.date)}
                position={positionOf(index, columns.length)}
                rows={rows.map((segment) => ({ key: segment.key, label: segment.label, ...(isSingle ? {} : { color: segment.color }), value: format(segment.value) }))}
                {...(hasTotal ? { total: format(total) } : {})}
              />
            </div>
          )
        })}
      </div>
    </ChartFrame>
  )
}
