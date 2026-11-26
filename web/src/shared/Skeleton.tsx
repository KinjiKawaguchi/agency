import type { ReactNode } from 'react'
import { useI18n } from '../i18n'
import './skeleton.css'

/** A placeholder bar. `kind` is its shape: a line of text, a short one, a title, or a box. */
export function Skeleton({ kind = 'line' }: { kind?: 'line' | 'short' | 'title' | 'box' }) {
  return <span className={`skeleton ${kind}`} aria-hidden="true" />
}

/** What stands in a region while it loads: the placeholders, announced to a screen reader once as "Loading…". */
export function Loading({ children }: { children: ReactNode }) {
  const { t } = useI18n()
  return (
    <div role="status" aria-busy="true">
      <span className="sr-only">{t.loading.label}</span>
      {children}
    </div>
  )
}
