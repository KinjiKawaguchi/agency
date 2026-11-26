import { useI18n } from '../i18n'
import { ChartFrame } from './ChartFrame'
import { positionOf } from './series'
import { Tip } from './Tip'

export interface Line {
  key: string
  label: string
  color: string
  /** One value per day; `null` where there is nothing to say, which breaks the line. */
  values: readonly (number | null)[]
}

interface TrendChartProps {
  dates: readonly string[]
  lines: readonly Line[]
  max: number
  format: (value: number) => string
  label: string
  noData: string
}

/** x of a day's center, as a share of the plot's width: the same slots as the columns of the other charts. */
const centerOf = (index: number, count: number): number => ((index + 0.5) / count) * 100

/** The path of a line in a 0-100 box; a missing value starts a new stroke. */
function pathOf(values: readonly (number | null)[], max: number): string {
  let previousWasMissing = true
  return values
    .map((value, index) => {
      if (value === null) {
        previousWasMissing = true
        return ''
      }
      const command = previousWasMissing ? 'M' : 'L'
      previousWasMissing = false
      return `${command}${centerOf(index, values.length).toFixed(2)} ${(100 - Math.min(value / max, 1) * 100).toFixed(2)}`
    })
    .join('')
}

const lastIndexWithValue = (values: readonly (number | null)[]): number => values.findLastIndex((value) => value !== null)

/** Lines over days, with a marker on each line's last value and a crosshair that tells every line's value for the day. */
export function TrendChart({ dates, lines, max, format, label, noData }: TrendChartProps) {
  const { fmt } = useI18n()
  return (
    <ChartFrame max={max} format={format} dates={dates} label={label}>
      <svg className="ev-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        {lines.map((line) => (
          <path key={line.key} className="ev-line" d={pathOf(line.values, max)} style={{ stroke: line.color }} />
        ))}
      </svg>
      {lines.map((line) => {
        const index = lastIndexWithValue(line.values)
        const value = line.values[index]
        return value === null || value === undefined ? null : (
          <i key={line.key} className="ev-end" style={{ left: `${centerOf(index, dates.length)}%`, bottom: `${Math.min(value / max, 1) * 100}%`, background: line.color }} />
        )
      })}
      <div className="ev-cells">
        {dates.map((date, index) => (
          <div key={date} className="ev-cell" tabIndex={0} aria-label={`${fmt.shortDate(date)}: ${lines.map((line) => `${line.label} ${valueText(line.values[index] ?? null, format, noData)}`).join(', ')}`}>
            <Tip title={fmt.shortDate(date)} position={positionOf(index, dates.length)} rows={lines.map((line) => ({ key: line.key, label: line.label, color: line.color, value: valueText(line.values[index] ?? null, format, noData) }))} />
          </div>
        ))}
      </div>
    </ChartFrame>
  )
}

const valueText = (value: number | null, format: (value: number) => string, noData: string): string => (value === null ? noData : format(value))
