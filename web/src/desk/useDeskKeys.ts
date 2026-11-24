import { useEffect, useRef } from 'react'

export interface DeskKeyActions {
  /** Moves the open document by one place. */
  onMove: (step: 1 | -1) => void
  /** Picks the nth option (1-based) of the open document. */
  onPick: (position: number) => void
  onSend: () => void
}

/** True while focus is in a control that takes typed text. A focused radio does not count. */
function isTextField(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true
  return target instanceof HTMLInputElement && target.type !== 'radio'
}

/** j and k move through the pile, 1-9 pick an option, Cmd/Ctrl+Enter sends. */
export function useDeskKeys(actions: DeskKeyActions): void {
  const latest = useRef(actions)
  useEffect(() => {
    latest.current = actions
  })

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
        event.preventDefault()
        latest.current.onSend()
        return
      }
      if (event.metaKey || event.ctrlKey || event.altKey) return
      if (isTextField(event.target)) return
      if (event.key === 'j' || event.key === 'k') {
        event.preventDefault()
        latest.current.onMove(event.key === 'j' ? 1 : -1)
      } else if (/^[1-9]$/.test(event.key)) {
        latest.current.onPick(Number(event.key))
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])
}
