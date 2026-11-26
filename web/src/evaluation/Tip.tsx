export interface TipRow {
  key: string
  label: string
  /** A CSS color for the swatch; absent when the row is the only one and the chart's title names it. */
  color?: string
  value: string
}

/** What a day says when hovered or focused. The swatch carries identity; the text stays in the text colors. */
export function Tip({ title, rows, total, position }: { title: string; rows: readonly TipRow[]; total?: string; position: number }) {
  // The tip's own `position` (0 at the first day, 1 at the last) is the share of its width that lies left of the day it explains, so it never leaves the plot.
  return (
    <div className="ev-tip" style={{ transform: `translateX(-${position * 100}%)` }} aria-hidden="true">
      <b>{title}</b>
      {rows.map((row) => (
        <span key={row.key} className="ev-tip-row">
          {row.color === undefined ? null : <i style={{ background: row.color }} />}
          {row.label}
          <b>{row.value}</b>
        </span>
      ))}
      {total === undefined ? null : <span className="ev-tip-row total">{total}</span>}
    </div>
  )
}
