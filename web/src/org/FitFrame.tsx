import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'

/** The smallest the chart is drawn: below it the text is too small to read, so the frame scrolls sideways instead. */
export const MIN_SCALE = 0.6

/** The scale that makes a chart of `chart` pixels fit a frame of `frame`, between the floor and 1; 1 when either is not known. */
export function fitScale(frame: number, chart: number): number {
  if (!(frame > 0) || !(chart > 0)) return 1
  return Math.min(1, Math.max(MIN_SCALE, frame / chart))
}

const PHONE = '(max-width: 720px)'

/**
 * A frame that draws a wide chart smaller to fit, so that the whole of it is seen at once, down to a floor;
 * past the floor it scrolls sideways inside itself and starts with its middle in view. On a phone the chart is
 * a list and is never scaled. It re-fits when the frame or the chart changes size, and re-centers when
 * `resetKey` changes.
 */
export function FitFrame({ resetKey, children }: { resetKey: string; children: ReactNode }) {
  const frame = useRef<HTMLDivElement>(null)
  const chart = useRef<HTMLDivElement>(null)
  const [fit, setFit] = useState({ scale: 1, width: 0, height: 0 })

  useLayoutEffect(() => {
    const frameElement = frame.current
    const chartElement = chart.current
    if (frameElement === null || chartElement === null) return undefined
    const measure = (): void => {
      // The chart's own layout size: transforms do not change it, so it is the size at scale 1.
      const width = Math.max(chartElement.scrollWidth, chartElement.clientWidth)
      const height = chartElement.scrollHeight
      const isPhone = typeof window.matchMedia === 'function' && window.matchMedia(PHONE).matches
      const scale = isPhone ? 1 : fitScale(frameElement.clientWidth, width)
      setFit((previous) => (previous.scale === scale && previous.width === width && previous.height === height ? previous : { scale, width, height }))
    }
    measure()
    if (typeof ResizeObserver === 'undefined') return undefined
    const observer = new ResizeObserver(measure)
    observer.observe(frameElement)
    observer.observe(chartElement)
    return () => observer.disconnect()
  }, [])

  useLayoutEffect(() => {
    const element = frame.current
    if (element !== null) element.scrollLeft = (element.scrollWidth - element.clientWidth) / 2
  }, [resetKey, fit.scale, fit.width])

  const isScaled = fit.scale < 1
  return (
    <div ref={frame} className="oc-frame">
      <div className="oc-fit" style={isScaled ? { width: fit.width * fit.scale, height: fit.height * fit.scale } : undefined}>
        <div ref={chart} className="oc-scaled" style={isScaled ? { transform: `scale(${fit.scale})` } : undefined}>
          {children}
        </div>
      </div>
    </div>
  )
}
