import type { PersonKind } from '../api/teamview'
import type { Presence } from './state'
import { hueOf, initials } from './identity'
import './face.css'

export type FaceSize = 'xs' | 'sm' | 'md' | 'lg'

interface FaceProps {
  label: string
  kind: PersonKind
  /** Draws the ring that shows what the person is doing. Omit for a plain face. */
  state?: Presence
  size?: FaceSize
  /** Hover text. The face is hidden from assistive technology; put the name in text next to it. */
  title?: string
}

/** A person's face: round for a human, a rounded square for an agent. The ring shows their state. */
export function Face({ label, kind, state, size = 'md', title }: FaceProps) {
  const classes = ['face', kind, `hue-${hueOf(label)}`, `size-${size}`, state === undefined ? '' : `ring-${state}`]
  return (
    <span className={classes.filter(Boolean).join(' ')} title={title ?? label} aria-hidden="true">
      {initials(label)}
    </span>
  )
}
