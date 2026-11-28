// The features of the Client API that the mock node answers, one module each. This list is the only place a feature is named for the mock node: the options
// it takes, the capabilities it says, the routes it answers, the routes a hosted member is refused, and how a running node is asked for it are all asked of
// the modules. Adding a feature is a module and a line here.
import { budgetFeature } from './budget.ts'
import type { MockFeature, ResolvedSettings } from './feature.ts'
import { historyFeature } from './history.ts'
import { membersFeature } from './members.ts'

/** The features, each with what it says of itself. */
const FEATURES = [budgetFeature, historyFeature, membersFeature] as const

/** The options of every feature, as one object type (each module declares its own). */
type UnionToIntersection<U> = (U extends unknown ? (value: U) => void : never) extends (value: infer I) => void ? I : never
type OptionsOf<F> = F extends MockFeature<infer Options> ? Options : never

/** What each feature of the list takes of a node's options. */
export type FeatureOptions = UnionToIntersection<OptionsOf<(typeof FEATURES)[number]>>

/** What an end-to-end test may ask of the node for the features: each feature says how it is asked for, in its own terms. */
export type AskedOfFeatures = UnionToIntersection<(typeof FEATURES)[number] extends infer F ? (F extends { toQuery: (asked: infer Asked) => unknown } ? Asked : never) : never>

const features: ReadonlyArray<MockFeature<FeatureOptions, AskedOfFeatures>> = FEATURES

/** The capabilities the node lists for the features it has. */
export const capabilitiesOf = (options: FeatureOptions): string[] => features.filter((feature) => feature.select(options) !== undefined).map((feature) => feature.capability)

/** The answer of the first feature that has this request's route and is on, or nothing. */
export function answerOf(request: Request, url: URL, options: FeatureOptions, context: { now: Date }): Response | undefined {
  for (const feature of features) {
    const answer = feature.select(options)
    if (answer === undefined) continue
    const answered = answer(request, url, context)
    if (answered !== undefined) return answered
  }
  return undefined
}

/** The options the features take, read from the query of a reset of the running mock node. */
export const optionsFromQuery = (query: URLSearchParams | undefined, resolved: ResolvedSettings): FeatureOptions =>
  Object.assign({}, ...features.map((feature) => feature.fromQuery(query, resolved))) as FeatureOptions

/** The query of a reset of the running mock node that asks for the features, from what an end-to-end test asked: the other half of `optionsFromQuery`. */
export const queryOfFeatures = (asked: AskedOfFeatures): Array<[string, string]> => features.flatMap((feature) => feature.toQuery(asked))

/** Whether a path is one of the owner's, which a hosted member is refused. */
export const isOwnerRoute = (pathname: string): boolean => features.some((feature) => feature.isOwnerRoute(pathname))
