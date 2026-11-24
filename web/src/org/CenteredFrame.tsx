import { useLayoutEffect, useRef, type ReactNode } from 'react'

/**
 * A sideways-scrolling frame that starts with its middle in view, where the top of an org chart is.
 * It re-centers when `resetKey` changes, such as when the chart focuses on another department.
 */
export function CenteredFrame({ resetKey, children }: { resetKey: string; children: ReactNode }) {
  const frame = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    const element = frame.current
    if (element !== null) element.scrollLeft = (element.scrollWidth - element.clientWidth) / 2
  }, [resetKey])
  return (
    <div ref={frame} className="oc-frame">
      {children}
    </div>
  )
}
