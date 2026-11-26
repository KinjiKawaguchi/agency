import { buildChartSvg, pageBackground, pixelRatioFor } from './exportImage'

/** A chart root of a given layout size; jsdom does no layout, so the size is set by hand. */
function chartOf(html: string, width: number, height: number): HTMLElement {
  const root = document.createElement('div')
  root.className = 'orgchart'
  root.style.color = 'red'
  root.innerHTML = html
  Object.defineProperty(root, 'offsetWidth', { value: width })
  Object.defineProperty(root, 'offsetHeight', { value: height })
  document.body.appendChild(root)
  return root
}

afterEach(() => void (document.body.innerHTML = ''))

describe('buildChartSvg', () => {
  const none = () => undefined

  it('puts the whole chart in an SVG at its natural size, with padding, over the page’s background', () => {
    const { svg, width, height } = buildChartSvg(chartOf('<p>Acme</p>', 800, 300), { background: 'rgb(12, 13, 16)', pseudoStyle: none })
    expect([width, height]).toEqual([848, 348])
    expect(svg).toContain('viewBox="0 0 848 348"')
    expect(svg).toContain('<rect width="100%" height="100%" fill="rgb(12, 13, 16)"/>')
    expect(svg).toContain('<foreignObject x="24" y="24" width="800" height="300">')
    expect(svg).toContain('xmlns="http://www.w3.org/1999/xhtml"')
  })

  it('writes the computed style of every element inline and drops the classes, which the image has no sheet for', () => {
    const { svg } = buildChartSvg(chartOf('<p style="font-weight: 700">Acme</p>', 100, 50), { background: '#fff', pseudoStyle: none })
    expect(svg).toMatch(/style="[^"]*color:\s*(red|rgb\(255, 0, 0\))/)
    expect(svg).toMatch(/<p[^>]*style="[^"]*font-weight:\s*700/)
    expect(svg).not.toContain('class=')
  })

  it('turns a pseudo-element that draws something into a real element at its place, and ignores one that draws nothing', () => {
    const pseudo = (_element: Element, which: '::before' | '::after') => (which === '::before' ? 'position:absolute;border-top:1px solid red' : undefined)
    const { svg } = buildChartSvg(chartOf('<ul><li>One</li></ul>', 100, 50), { background: '#fff', pseudoStyle: pseudo })
    // The root, the list, and the item each get one `::before` span, first among their children.
    expect(svg.match(/<span style="position:absolute;border-top:1px solid red"/g)).toHaveLength(3)
    expect(svg.indexOf('<span style="position:absolute')).toBeLessThan(svg.indexOf('One'))
  })

  it('escapes text so that the SVG stays well formed', () => {
    const { svg } = buildChartSvg(chartOf('<p>R&amp;D &lt;lab&gt;</p>', 100, 50), { background: '#fff', pseudoStyle: none })
    expect(new DOMParser().parseFromString(svg, 'image/svg+xml').querySelector('parsererror')).toBeNull()
    expect(svg).toContain('R&amp;D &lt;lab&gt;')
  })
})

describe('pixelRatioFor', () => {
  it('is 2 for an ordinary chart and smaller where the canvas would be too large', () => {
    expect(pixelRatioFor(1200, 800)).toBe(2)
    expect(pixelRatioFor(10_000, 4_000)).toBeCloseTo(Math.sqrt(16_000_000 / 40_000_000))
    expect(pixelRatioFor(40_000, 100)).toBeCloseTo(16_000 / 40_000)
  })
})

describe('pageBackground', () => {
  it('takes the first background up the tree that is not see-through, and white when there is none', () => {
    const outer = document.createElement('div')
    outer.style.backgroundColor = 'rgb(1, 2, 3)'
    const inner = document.createElement('div')
    outer.appendChild(inner)
    document.body.appendChild(outer)
    expect(pageBackground(inner)).toBe('rgb(1, 2, 3)')
    document.body.innerHTML = ''
    expect(pageBackground(document.createElement('div'))).toBe('#ffffff')
  })
})
