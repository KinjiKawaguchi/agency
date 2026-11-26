import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { useI18n } from '../i18n'
import { useToast } from '../shared/Toasts'
import { buildChartSvg, download, pageBackground, svgToBlob, svgToPng } from './exportImage'
import { MAX_ZOOM, MIN_ZOOM, scaleOf, steppedScale, type ZoomMode } from './zoom'

const PHONE = '(max-width: 720px)'

// Read at each render: a resize makes the ResizeObserver below re-render, so a phone turned sideways is noticed.
const isPhone = (): boolean => typeof window.matchMedia === 'function' && window.matchMedia(PHONE).matches

interface Props {
  /** Changes when a different chart is shown, so that the view starts again, centered. */
  resetKey: string
  /** What the downloaded files are called, without the extension. */
  fileName: string
  children: ReactNode
}

/**
 * The org chart in a frame that is as wide as the window: drawn smaller to fit on its own (down to a floor),
 * with buttons to fit it to the width exactly, zoom in and out, and download the whole chart as an image,
 * not only the part in view. On a phone the chart is a list and is never scaled.
 */
export function OrgChartViewer({ resetKey, fileName, children }: Props) {
  const { t } = useI18n()
  const toast = useToast()
  const frame = useRef<HTMLDivElement>(null)
  const chart = useRef<HTMLDivElement>(null)
  const [mode, setMode] = useState<ZoomMode>({ kind: 'auto' })
  const [natural, setNatural] = useState({ frame: 0, width: 0, height: 0 })
  const [isBusy, setIsBusy] = useState(false)
  // The point of the chart (in its own, unscaled pixels) that was in the middle of the frame when the viewer zoomed.
  const anchor = useRef<number | undefined>(undefined)

  useLayoutEffect(() => {
    const frameElement = frame.current
    const chartElement = chart.current
    if (frameElement === null || chartElement === null) return undefined
    const measure = (): void => {
      // The chart's own layout size: transforms do not change it, so it is the size at 100%.
      const width = Math.max(chartElement.scrollWidth, chartElement.clientWidth)
      const height = chartElement.scrollHeight
      setNatural((previous) => (previous.frame === frameElement.clientWidth && previous.width === width && previous.height === height ? previous : { frame: frameElement.clientWidth, width, height }))
    }
    measure()
    if (typeof ResizeObserver === 'undefined') return undefined
    const observer = new ResizeObserver(measure)
    observer.observe(frameElement)
    observer.observe(chartElement)
    return () => observer.disconnect()
  }, [])

  const scale = isPhone() ? 1 : scaleOf(mode, natural.frame, natural.width)

  useLayoutEffect(() => {
    const element = frame.current
    if (element === null) return
    // A zoom keeps the middle of what the viewer was looking at in the middle; anything else starts from the chart's middle.
    element.scrollLeft = anchor.current === undefined ? (element.scrollWidth - element.clientWidth) / 2 : anchor.current * scale - element.clientWidth / 2
    anchor.current = undefined
  }, [resetKey, scale, natural.width])

  const zoomTo = (next: ZoomMode): void => {
    const element = frame.current
    if (element !== null) anchor.current = (element.scrollLeft + element.clientWidth / 2) / scale
    setMode(next)
  }

  // A different chart starts in automatic fit again.
  useLayoutEffect(() => setMode({ kind: 'auto' }), [resetKey])

  const save = async (format: 'png' | 'svg'): Promise<void> => {
    const root = chart.current?.firstElementChild
    if (!(root instanceof HTMLElement)) return
    setIsBusy(true)
    try {
      const drawn = buildChartSvg(root, { background: pageBackground() })
      if (format === 'svg') download(svgToBlob(drawn), `${fileName}.svg`)
      else download(await svgToPng(drawn), `${fileName}.png`)
    } catch (error) {
      toast(t.org.view.downloadFailed(format, error instanceof Error ? error.message : String(error)), 'error')
    } finally {
      setIsBusy(false)
    }
  }

  const isScaled = scale !== 1
  return (
    <>
      <div className="oc-tools" role="toolbar" aria-label={t.org.view.toolbar}>
        <div className="oc-zoom" role="group" aria-label={t.org.view.zoom}>
          <button type="button" className="btn small" onClick={() => setMode({ kind: 'fit' })}>{t.org.view.fit}</button>
          <button type="button" className="btn small" aria-label={t.org.view.zoomOut} disabled={scale <= MIN_ZOOM} onClick={() => zoomTo({ kind: 'zoom', scale: steppedScale(scale, 'out') })}>−</button>
          <button type="button" className="btn small oc-percent" aria-label={t.org.view.actualSize} onClick={() => zoomTo({ kind: 'zoom', scale: 1 })}>{Math.round(scale * 100)}%</button>
          <button type="button" className="btn small" aria-label={t.org.view.zoomIn} disabled={scale >= MAX_ZOOM} onClick={() => zoomTo({ kind: 'zoom', scale: steppedScale(scale, 'in') })}>+</button>
        </div>
        <div className="oc-download" role="group" aria-label={t.org.view.download}>
          <span className="muted">{t.org.view.download}</span>
          <button type="button" className="btn small" disabled={isBusy} title={t.org.view.pngHint} onClick={() => void save('png')}>PNG</button>
          <button type="button" className="btn small" disabled={isBusy} title={t.org.view.svgHint} onClick={() => void save('svg')}>SVG</button>
        </div>
      </div>
      <div ref={frame} className="oc-frame oc-wide">
        <div className="oc-fit" style={isScaled ? { width: natural.width * scale, height: natural.height * scale } : undefined}>
          <div ref={chart} className="oc-scaled" style={isScaled ? { transform: `scale(${scale})` } : undefined}>
            {children}
          </div>
        </div>
      </div>
    </>
  )
}
