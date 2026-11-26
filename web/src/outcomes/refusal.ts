import type { Reading } from '../api/outcomesApi'
import type { Messages } from '../i18n/en'

/**
 * What a reading says when it has no numbers: why, and what is missing. `ready` has none to say. A state that needs a
 * detail (a day, a count) the answer does not carry is said to be one this version cannot show, and nothing is guessed.
 */
export function refusalOf(reading: Reading, states: Messages['outcomes']['readings']['states'], date: (day: string) => string): string | undefined {
  switch (reading.state) {
    case 'ready':
      return undefined
    case 'notShipped':
      return states.notShipped
    case 'shipDateUnknown':
      return states.shipDateUnknown
    case 'noBaseline':
      return reading.shippedOn === undefined ? states.unknown : states.noBaseline(date(reading.shippedOn))
    case 'tooEarly':
      return reading.readableFrom === undefined ? states.unknown : states.tooEarly(date(reading.readableFrom))
    case 'noFollowUp':
      return states.noFollowUp
    case 'tooFew':
      return reading.points === undefined || reading.needs === undefined ? states.unknown : states.tooFew(reading.points, reading.needs)
    case 'unknown':
      return states.unknown
  }
}
