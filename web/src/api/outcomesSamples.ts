// The shared samples of the outcomes routes (made-up figures, kept in `testdata/`), for the mock node and the tests.
import outcomes from '../../../testdata/outcomes.sample.json' with { type: 'json' }
import readings from '../../../testdata/readings.sample.json' with { type: 'json' }

export const sampleOutcomes: unknown = outcomes
export const sampleReadings: unknown = readings
