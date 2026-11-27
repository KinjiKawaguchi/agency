import { readFileSync } from 'node:fs'
import { join } from 'node:path'

/** The set of characters the screens print as a mark, and the cases to test it with: one file that the node and the other clients read too. */
export const sharedHidden = JSON.parse(readFileSync(join(process.cwd(), '..', 'testdata', 'label-hidden-chars.json'), 'utf8')) as {
  mark: string
  hidden: Array<[string, string]>
  cases: Array<{ text: string; printable: string }>
  /** A text an agent or a person wrote hides `hidden` minus these (a tab and the line breaks stay), and the cases of such texts. */
  textKeeps: string[]
  textCases: Array<{ text: string; printable: string }>
}

/** The inclusive ranges of code points of the set. */
export const hiddenRanges: ReadonlyArray<readonly [number, number]> = sharedHidden.hidden.map(([from, to]) => [Number.parseInt(from, 16), Number.parseInt(to, 16)] as const)
