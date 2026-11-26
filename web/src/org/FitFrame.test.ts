import { fitScale, MIN_SCALE } from './FitFrame'

describe('fitScale', () => {
  it('leaves a chart that fits as it is', () => {
    expect(fitScale(1200, 900)).toBe(1)
    expect(fitScale(1200, 1200)).toBe(1)
  })

  it('shrinks a wider chart by frame over chart', () => {
    expect(fitScale(1000, 1250)).toBeCloseTo(0.8)
  })

  it('stops at the floor, past which the frame scrolls', () => {
    expect(fitScale(1000, 3000)).toBe(MIN_SCALE)
  })

  it('draws at full size when either width is not known, as before the first measure', () => {
    expect(fitScale(0, 900)).toBe(1)
    expect(fitScale(900, 0)).toBe(1)
  })
})
