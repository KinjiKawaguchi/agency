import { AUTO_FLOOR, fitScale, MAX_ZOOM, MIN_ZOOM, scaleOf, steppedScale } from './zoom'

describe('fitScale', () => {
  it('leaves a chart that fits as it is', () => {
    expect(fitScale(1200, 900)).toBe(1)
    expect(fitScale(1200, 1200)).toBe(1)
  })

  it('shrinks a wider chart by frame over chart, down to the floor', () => {
    expect(fitScale(1000, 1250)).toBeCloseTo(0.8)
    expect(fitScale(1000, 3000)).toBe(AUTO_FLOOR)
    expect(fitScale(1000, 3000, MIN_ZOOM)).toBeCloseTo(1 / 3)
  })

  it('draws at full size when either width is not known, as before the first measure', () => {
    expect(fitScale(0, 900)).toBe(1)
    expect(fitScale(900, 0)).toBe(1)
  })
})

describe('scaleOf', () => {
  it('fits on its own down to the floor, exactly to the width on request, and as the viewer zoomed', () => {
    expect(scaleOf({ kind: 'auto' }, 1000, 3000)).toBe(AUTO_FLOOR)
    expect(scaleOf({ kind: 'fit' }, 1000, 3000)).toBeCloseTo(1 / 3)
    expect(scaleOf({ kind: 'zoom', scale: 1.5 }, 1000, 3000)).toBe(1.5)
  })

  it('keeps a zoom inside the range', () => {
    expect(scaleOf({ kind: 'zoom', scale: 9 }, 1000, 1000)).toBe(MAX_ZOOM)
    expect(scaleOf({ kind: 'zoom', scale: 0.01 }, 1000, 1000)).toBe(MIN_ZOOM)
  })
})

describe('steppedScale', () => {
  it('steps by a quarter in whole percent, and stops at the ends', () => {
    expect(steppedScale(1, 'in')).toBe(1.25)
    expect(steppedScale(1, 'out')).toBe(0.8)
    expect(steppedScale(MAX_ZOOM, 'in')).toBe(MAX_ZOOM)
    expect(steppedScale(MIN_ZOOM, 'out')).toBe(MIN_ZOOM)
  })
})
