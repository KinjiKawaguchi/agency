import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { en } from '../i18n/en'
import { ja } from '../i18n/ja'

// The windows of a reading are the protocol's (crates/agency-proto/src/reading.rs), so the sentences that name them are checked
// against that file: a change of a window there fails here until the words say the same.
const rust = readFileSync(join(process.cwd(), '..', 'crates', 'agency-proto', 'src', 'reading.rs'), 'utf8')
const constant = (name: string): number => Number(new RegExp(`pub const ${name}: [a-z0-9]+ = (\\d+);`).exec(rust)?.[1])

describe('the numbers the readings’ sentences name', () => {
  const [settle, window, minimum] = [constant('SETTLE_DAYS'), constant('WINDOW_DAYS'), constant('MIN_POINTS')]

  it('are the protocol’s windows, in English and Japanese', () => {
    expect([settle, window, minimum]).toEqual([14, 90, 3])
    const states = en.outcomes.readings.states
    expect(states.noBaseline('D')).toContain(`${window} days before`)
    expect(states.noFollowUp).toContain(`${window} days after`)
    expect(states.tooEarly('D')).toContain(`${settle} days to show`)
    const japanese = ja.outcomes.readings.states
    expect(japanese.noBaseline('D')).toContain(`${window} 日`)
    expect(japanese.noFollowUp).toContain(`${window} 日`)
    expect(japanese.tooEarly('D')).toContain(`${settle} 日`)
  })

  it('say as few numbers as the node asks for: the count needed comes from the node’s answer, not from the words', () => {
    expect(en.outcomes.readings.states.tooFew(2, minimum)).toContain(`2 of ${minimum}`)
    expect(ja.outcomes.readings.states.tooFew(2, minimum)).toContain(`2 / ${minimum}`)
  })
})
