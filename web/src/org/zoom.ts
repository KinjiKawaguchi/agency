/** The smallest the chart is drawn on its own: below it the text is too small to read, so the frame scrolls sideways instead. */
export const AUTO_FLOOR = 0.6

/** The range the viewer can zoom to by hand. */
export const MIN_ZOOM = 0.2
export const MAX_ZOOM = 2

const STEP = 1.25

/** How the chart is sized: on its own to fit (down to a floor), exactly to the frame's width, or by the viewer's zoom. */
export type ZoomMode = { kind: 'auto' } | { kind: 'fit' } | { kind: 'zoom'; scale: number }

const clamp = (value: number, low: number, high: number): number => Math.min(high, Math.max(low, value))

/** The scale that makes a chart of `chart` pixels fit a frame of `frame`, between `floor` and 1; 1 when either is not known. */
export function fitScale(frame: number, chart: number, floor: number = AUTO_FLOOR): number {
  if (!(frame > 0) || !(chart > 0)) return 1
  return clamp(frame / chart, floor, 1)
}

/** The scale a mode gives for a frame and a chart of these widths. */
export function scaleOf(mode: ZoomMode, frame: number, chart: number): number {
  if (mode.kind === 'zoom') return clamp(mode.scale, MIN_ZOOM, MAX_ZOOM)
  return fitScale(frame, chart, mode.kind === 'fit' ? MIN_ZOOM : AUTO_FLOOR)
}

/** One step in or out from the scale now shown, in whole percent so that the figure on the button is the figure kept. */
export function steppedScale(current: number, direction: 'in' | 'out'): number {
  const next = direction === 'in' ? current * STEP : current / STEP
  return clamp(Math.round(next * 100) / 100, MIN_ZOOM, MAX_ZOOM)
}
