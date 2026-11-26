import { useRef, useState, type KeyboardEvent } from 'react'

/**
 * One tab stop for a chart of days, and the arrow keys between them: only the day that is current can be tabbed
 * to, Left and Right move to the previous and the next day, Home and End to the first and the last. The newest day
 * is the one the chart starts on. This is the "roving tabindex" of a list of items.
 */
export function useDays(count: number) {
  const [chosen, setChosen] = useState<number | undefined>(undefined)
  const items = useRef<Array<HTMLElement | null>>([])
  const current = Math.min(chosen ?? count - 1, Math.max(count - 1, 0))

  const move = (to: number): void => {
    const next = Math.min(Math.max(to, 0), count - 1)
    setChosen(next)
    items.current[next]?.focus()
  }

  const onKeyDown = (event: KeyboardEvent): void => {
    const target = { ArrowLeft: current - 1, ArrowRight: current + 1, Home: 0, End: count - 1 }[event.key]
    if (target === undefined) return
    event.preventDefault()
    move(target)
  }

  /** What each day's element needs: a place to be found by, whether it can be tabbed to, and to know when it is focused. */
  const itemProps = (index: number) => ({
    ref: (element: HTMLElement | null) => {
      items.current[index] = element
    },
    tabIndex: index === current ? 0 : -1,
    onFocus: () => setChosen(index),
  })

  return { onKeyDown, itemProps }
}
