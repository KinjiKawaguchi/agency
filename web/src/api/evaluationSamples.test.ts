import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { readEffort, readEvaluation } from './evaluationAnswers'

// The node's own samples (docs/protocol.md, "Evaluation") are what the client has to read, whatever the mock
// serves: if the node's answer changes shape, the sample changes with it and this fails.
// Found from this file, so it does not matter where the tests are started from.
const sample = (name: string): unknown => JSON.parse(readFileSync(join(import.meta.dirname, '..', '..', '..', 'testdata', name), 'utf8'))

describe('the node’s samples', () => {
  it('read as evaluation answers, unchanged: a valid answer needs no mending', () => {
    const answer = sample('evaluation.sample.json')
    expect(readEvaluation(answer)).toEqual(answer)
  })

  it('read as effort answers, unchanged', () => {
    const answer = sample('effort.sample.json')
    expect(readEffort(answer)).toEqual(answer)
  })

  it('give each team its own period, and name the teams that could not answer', () => {
    const evaluation = readEvaluation(sample('evaluation.sample.json'))
    expect(evaluation.teams.length).toBeGreaterThan(0)
    for (const team of evaluation.teams) expect(team.byDay.every((day) => day.date >= team.from && day.date <= team.until)).toBe(true)
    expect(Array.isArray(evaluation.unavailable)).toBe(true)
  })
})
