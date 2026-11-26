import type { ReactNode } from 'react'
import { useI18n } from '../i18n'
import './skeleton.css'

/** A placeholder bar. `kind` is its shape: a line of text, a short one, a title, or a box. */
export function Skeleton({ kind = 'line' }: { kind?: 'line' | 'short' | 'title' | 'box' }) {
  return <span className={`skeleton ${kind}`} aria-hidden="true" />
}

/**
 * What stands in a region while it loads: the placeholders, announced to a screen reader once as "Loading…".
 * Placeholders inside a page that is already there (a team's card among the others) are `quiet`: they only say
 * they are busy, so that a page of many does not announce each.
 */
export function Loading({ children, quiet = false }: { children: ReactNode; quiet?: boolean }) {
  const { t } = useI18n()
  if (quiet) return <div aria-busy="true">{children}</div>
  return (
    <div role="status" aria-busy="true">
      <span className="sr-only">{t.loading.label}</span>
      {children}
    </div>
  )
}
