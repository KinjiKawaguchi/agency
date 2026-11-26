// The whole org chart as an image, in the theme the page is in. The chart is HTML and CSS, so it is copied
// into an SVG `foreignObject` with every element's computed style written inline (the colors are already
// worked out for the current theme) and its connector lines, which CSS draws with ::before and ::after,
// made into real elements. The SVG is the "SVG" download; drawn onto a canvas it is the PNG.

const SVG_NS = 'http://www.w3.org/2000/svg'
const PADDING = 24

/** What a pseudo-element looks like, as its style text, or nothing when it draws nothing. */
export type PseudoStyle = (element: Element, pseudo: '::before' | '::after') => string | undefined

/** The properties that decide how the chart looks and lays out; the rest of the computed style is the browser's defaults and would only make the file large. */
const PROPERTIES = [
  'display', 'position', 'top', 'right', 'bottom', 'left', 'z-index', 'box-sizing', 'float',
  'width', 'height', 'min-width', 'min-height', 'max-width', 'max-height',
  'margin-top', 'margin-right', 'margin-bottom', 'margin-left',
  'padding-top', 'padding-right', 'padding-bottom', 'padding-left',
  'border-top-width', 'border-right-width', 'border-bottom-width', 'border-left-width',
  'border-top-style', 'border-right-style', 'border-bottom-style', 'border-left-style',
  'border-top-color', 'border-right-color', 'border-bottom-color', 'border-left-color',
  'border-top-left-radius', 'border-top-right-radius', 'border-bottom-right-radius', 'border-bottom-left-radius',
  'background-color', 'background-image', 'box-shadow', 'opacity', 'transform', 'transform-origin',
  'color', 'font-family', 'font-size', 'font-weight', 'font-style', 'font-variant-numeric', 'line-height', 'letter-spacing',
  'text-align', 'text-decoration-line', 'text-transform', 'text-overflow', 'white-space', 'overflow-wrap', 'word-break', 'vertical-align',
  'flex-direction', 'flex-wrap', 'flex-grow', 'flex-shrink', 'flex-basis', 'order', 'align-items', 'align-self', 'align-content', 'justify-content', 'justify-items', 'gap', 'row-gap', 'column-gap',
  'overflow-x', 'overflow-y', 'list-style-type', 'list-style-position', 'content',
] as const

const styleText = (computed: CSSStyleDeclaration): string =>
  PROPERTIES.flatMap((name) => {
    const value = computed.getPropertyValue(name)
    return value === '' ? [] : [`${name}:${value}`]
  }).join(';')

/** The style of a pseudo-element that has content, which is the only kind CSS draws; `none` and `normal` draw nothing. */
export const browserPseudoStyle: PseudoStyle = (element, pseudo) => {
  const computed = getComputedStyle(element, pseudo)
  const content = computed.getPropertyValue('content')
  return content === '' || content === 'none' || content === 'normal' ? undefined : styleText(computed)
}

/** A copy of `source` with its computed style inline all the way down, and its pseudo-elements as elements. */
function inlineCopy(source: Element, pseudoStyle: PseudoStyle): Element {
  const copy = source.cloneNode(false) as Element
  copy.setAttribute('style', styleText(getComputedStyle(source)))
  copy.removeAttribute('class')
  const before = pseudoStyle(source, '::before')
  if (before !== undefined) copy.appendChild(pseudoElement(source.ownerDocument, before))
  for (const child of Array.from(source.childNodes)) {
    copy.appendChild(child.nodeType === 1 ? inlineCopy(child as Element, pseudoStyle) : child.cloneNode(true))
  }
  const after = pseudoStyle(source, '::after')
  if (after !== undefined) copy.appendChild(pseudoElement(source.ownerDocument, after))
  return copy
}

function pseudoElement(document: Document, style: string): Element {
  const element = document.createElement('span')
  element.setAttribute('style', style)
  return element
}

export interface ChartSvg {
  svg: string
  width: number
  height: number
}

export interface ChartSvgOptions {
  /** The page's background, so that the image has the colors of the theme and is not see-through. */
  background: string
  /** How a pseudo-element looks; the browser's computed style unless a test says otherwise. */
  pseudoStyle?: PseudoStyle
}

/** The chart as an SVG document at its natural size (the size it has at 100%, whatever its zoom on the page). */
export function buildChartSvg(root: HTMLElement, { background, pseudoStyle = browserPseudoStyle }: ChartSvgOptions): ChartSvg {
  const width = root.offsetWidth
  const height = root.offsetHeight
  // The serializer writes the XHTML namespace on the copy's root by itself.
  const copy = inlineCopy(root, pseudoStyle)
  const total = { width: width + 2 * PADDING, height: height + 2 * PADDING }
  const svg =
    `<svg xmlns="${SVG_NS}" width="${total.width}" height="${total.height}" viewBox="0 0 ${total.width} ${total.height}">` +
    `<rect width="100%" height="100%" fill="${background}"/>` +
    `<foreignObject x="${PADDING}" y="${PADDING}" width="${width}" height="${height}">${new XMLSerializer().serializeToString(copy)}</foreignObject>` +
    '</svg>'
  return { svg, ...total }
}

/** The background the page shows, looking up the tree for the first one that is not see-through. */
export function pageBackground(from: Element = document.body): string {
  for (let element: Element | null = from; element !== null; element = element.parentElement) {
    const color = getComputedStyle(element).backgroundColor
    if (color !== '' && color !== 'transparent' && color !== 'rgba(0, 0, 0, 0)') return color
  }
  return '#ffffff'
}

/** A canvas holds at most about this many pixels on the smallest phones, and no side may pass this many. */
const MAX_PIXELS = 16_000_000
const MAX_SIDE = 16_000

/** The pixel ratio for a sharp image that still fits the canvas limits: 2 where it can be, less for a very large chart. */
export function pixelRatioFor(width: number, height: number): number {
  return Math.min(2, Math.sqrt(MAX_PIXELS / (width * height)), MAX_SIDE / Math.max(width, height))
}

const toDataUrl = (svg: string): string => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`

/** Draws the SVG onto a canvas and returns it as a PNG. Rejects where the browser will not draw it or will not read the canvas back. */
export async function svgToPng({ svg, width, height }: ChartSvg): Promise<Blob> {
  const image = new Image()
  await new Promise<void>((resolve, reject) => {
    image.onload = () => resolve()
    image.onerror = () => reject(new Error('The browser could not draw the chart.'))
    image.src = toDataUrl(svg)
  })
  const ratio = pixelRatioFor(width, height)
  const canvas = document.createElement('canvas')
  canvas.width = Math.floor(width * ratio)
  canvas.height = Math.floor(height * ratio)
  const context = canvas.getContext('2d')
  if (context === null) throw new Error('The browser could not make an image.')
  context.scale(ratio, ratio)
  context.drawImage(image, 0, 0, width, height)
  return new Promise<Blob>((resolve, reject) => canvas.toBlob((blob) => (blob === null ? reject(new Error('The browser could not save the image.')) : resolve(blob)), 'image/png'))
}

export const svgToBlob = ({ svg }: ChartSvg): Blob => new Blob([svg], { type: 'image/svg+xml' })

/** Hands a file to the browser's downloads. */
export function download(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = fileName
  document.body.appendChild(link)
  link.click()
  link.remove()
  window.setTimeout(() => URL.revokeObjectURL(url), 10_000)
}
