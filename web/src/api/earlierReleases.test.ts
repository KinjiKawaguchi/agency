// What earlier nodes answered (`testdata/releases/<tag>/client/`, kept by `scripts/release/snapshot-data.sh`)
// must still fit what this client reads: a new web client is embedded in its own node, but the types it is built
// from are the ones an older node's answers are read through in the apps and in a federation's replicas, and a
// type that asks for more than an older node says is the break docs/compatibility.md rules out.
//
// The client reads most answers only through the generated types (`bindings/`), so the check is the compiler's:
// each file becomes an expression passed where the type is expected. A field the type lacks is ignored, as at
// runtime; a field it needs and the answer has not, or a value it does not allow, fails. The answers that have
// a reader of their own (`readActivity`, `readEvaluation`, `readMembers`) are also read with it.
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { readActivity } from './activityAnswers'
import { readEvaluation } from './evaluationAnswers'
import { readMembers } from './membersAnswers'

/** The generated type each kept file is read as, by its name in `testdata/client/`. */
const TYPE_OF: Record<string, string> = {
  activity: 'Activity',
  'decision-entry': 'DecisionEntry',
  'decision-log': 'DecisionLog',
  'decision-log-member': 'DecisionLog',
  'decision-result': 'DecisionResult',
  'decision-result-kept': 'DecisionResult',
  'decision-result-queued': 'DecisionResult',
  'device-change-result': 'DeviceChangeResult',
  'device-change-result-queued': 'DeviceChangeResult',
  'device-list': 'DeviceList',
  'device-list-p256': 'DeviceList',
  'device-request': 'DeviceRequest',
  evaluation: 'Evaluation',
  me: 'Me',
  'me-funding': 'Me',
  'me-hides-titles': 'Me',
  'me-home-nodes': 'Me',
  'me-member': 'Me',
  'me-root-changes': 'Me',
  org: 'OrgDefinition',
  'org-members': 'OrgMembers',
  'push-subscription': 'PushSubscription',
  'push-subscription-list': 'PushSubscriptionList',
  'push-token': 'PushToken',
  'push-token-list': 'PushTokenList',
  'push-vapid': 'VapidKey',
  session: 'Session',
  'team-snapshot-local': 'TeamSnapshot',
  'team-snapshot-replica': 'TeamSnapshot',
  usage: 'TokenUsage',
}
/**
 * Answers that do not fit today's type, and why. This list is exact twice over: an entry whose file fits now is
 * an error, so that it is removed when the difference is, and every complaint about the file must hold the
 * message the entry expects, so that another break in the same file is not taken for the known one.
 */
const KNOWN_DIFFERENCES: Record<string, { reason: string; expects: string }> = {
  'v0.3.0/push-token': {
    reason:
      '`id` was added after v0.3.0 (#453); the web client only reads its own node, and the Apple apps do not decode `id`. A client that reads other releases\' push tokens is when the type changes (AgencyCore, #580)',
    expects: "Property 'id' is missing",
  },
  'v0.3.0/push-token-list': {
    reason: 'the same: each token of the list has no `id` in v0.3.0',
    expects: "Property 'id' is missing",
  },
}
/** An error answer is `{ "error": text }`, which the client reads by hand and has no generated type for. */
const READ_BY_HAND = new Set(['error'])

const root = join(process.cwd(), '..')
const releasesDir = join(root, 'testdata', 'releases')

interface KeptAnswer {
  tag: string
  name: string
  text: string
}

function keptAnswers(): KeptAnswer[] {
  const answers: KeptAnswer[] = []
  for (const tag of readdirSync(releasesDir).sort()) {
    let files: string[]
    try {
      files = readdirSync(join(releasesDir, tag, 'client')).sort()
    } catch {
      continue
    }
    for (const file of files) {
      const text = readFileSync(join(releasesDir, tag, 'client', file), 'utf8')
      answers.push({ tag, name: file.replace(/\.json$/, ''), text })
    }
  }
  return answers
}

const PRELUDE = `type DeepWritable<T> = T extends readonly (infer U)[] ? DeepWritable<U>[] : T extends object ? { -readonly [K in keyof T]: DeepWritable<T[K]> } : T
declare function answer<T>(value: T): DeepWritable<T>
declare function reads<T>(value: T): void
reads<string>(5) // the canary: the compiler must complain of this line, or it did not run
`

/** One line for each answer, in order, so that the compiler's line number says which one failed. */
function program(answers: KeptAnswer[]): string {
  const types = [...new Set(answers.map(({ name }) => TYPE_OF[name]).filter((type) => type !== undefined))]
  const imports = types.map((type) => `import type { ${type} } from '${join(root, 'bindings', type)}'`)
  const lines = answers.map(({ name, text }) => {
    const type = TYPE_OF[name]
    return type === undefined ? '' : `reads<${type}>(answer(${JSON.stringify(JSON.parse(text))} as const))`
  })
  return [...imports, ...PRELUDE.trimEnd().split('\n'), ...lines].join('\n') + '\n'
}

interface Complaint {
  line: number
  text: string
}

/** The compiler's complaints as it wrote them, a message with its continuation lines in one. */
function complaintsOf(output: string): Complaint[] {
  const found: Complaint[] = []
  for (const line of output.split('\n')) {
    const at = /answers\.ts\((\d+),\d+\): error (.*)$/.exec(line)
    if (at) found.push({ line: Number(at[1]), text: at[2] ?? '' })
    else if (/^\s/.test(line) && found.length > 0) found[found.length - 1]!.text += ` ${line.trim()}`
  }
  return found
}

/** What `tsc` said, when it ran: it exits 1 or 2 when it has complaints and 0 when it has none. */
function compile(dir: string): string {
  const tsc = join(process.cwd(), 'node_modules', 'typescript', 'lib', 'tsc.js')
  try {
    return execFileSync(process.execPath, [tsc, '-p', dir, '--pretty', 'false'], { encoding: 'utf8', stdio: 'pipe' })
  } catch (error) {
    const failure = error as { status?: number; stdout?: string }
    if (failure.status === 1 || failure.status === 2) return failure.stdout ?? ''
    throw error
  }
}

/** The compiler's complaints about the answers, by the answer they are about: `<tag>/<name>`. */
function typeErrors(answers: KeptAnswer[]): { answer: string; text: string }[] {
  const dir = mkdtempSync(join(tmpdir(), 'earlier-releases-'))
  try {
    const source = program(answers)
    writeFileSync(join(dir, 'answers.ts'), source)
    writeFileSync(
      join(dir, 'tsconfig.json'),
      JSON.stringify({
        compilerOptions: {
          target: 'ES2022',
          module: 'ESNext',
          moduleResolution: 'bundler',
          strict: true,
          noEmit: true,
          skipLibCheck: true,
          types: [],
        },
        files: ['answers.ts'],
      }),
    )
    const complaints = complaintsOf(compile(dir))
    const firstAnswerLine = source.split('\n').length - answers.length
    if (!complaints.some(({ line }) => line === firstAnswerLine - 1)) {
      throw new Error('the compiler did not complain of the canary line: it did not check the answers')
    }
    return complaints
      .filter(({ line }) => line >= firstAnswerLine)
      .map(({ line, text }) => {
        const kept = answers[line - firstAnswerLine]
        return { answer: kept ? `${kept.tag}/${kept.name}` : 'program', text }
      })
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

describe('the answers of earlier nodes, read with this client', () => {
  const answers = keptAnswers()

  it('are kept, and each has a type it is read as', () => {
    expect(answers.map(({ tag }) => tag)).toContain('v0.3.0')
    const unknown = answers.filter(({ name }) => !(name in TYPE_OF) && !READ_BY_HAND.has(name)).map(({ tag, name }) => `${tag}/${name}`)
    expect(unknown, 'add the file to TYPE_OF with the type the client reads it as').toEqual([])
  })

  it('fit the generated types, except the differences named above', () => {
    const errors = typeErrors(answers)
    const unexpected = errors.filter(({ answer, text }) => !text.includes(KNOWN_DIFFERENCES[answer]?.expects ?? '\u0000'))
    expect(unexpected).toEqual([])
    const gone = Object.keys(KNOWN_DIFFERENCES).filter((key) => !errors.some(({ answer }) => answer === key))
    expect(gone, 'these answers fit now: remove them from KNOWN_DIFFERENCES').toEqual([])
  })

  it('are read by the readers that check an answer', () => {
    const named = (name: string) => answers.filter((answer) => answer.name === name)
    expect(named('activity').length).toBeGreaterThan(0)
    for (const { text } of named('activity')) expect(() => readActivity(JSON.parse(text))).not.toThrow()
    expect(named('evaluation').length).toBeGreaterThan(0)
    for (const { text } of named('evaluation')) expect(() => readEvaluation(JSON.parse(text))).not.toThrow()
    expect(named('org-members').length).toBeGreaterThan(0)
    for (const { text } of named('org-members')) expect(() => readMembers(JSON.parse(text))).not.toThrow()
  })

  it('an error answer names its reason', () => {
    for (const { text } of answers.filter(({ name }) => name === 'error')) {
      expect(typeof (JSON.parse(text) as { error: unknown }).error).toBe('string')
    }
  })
})
