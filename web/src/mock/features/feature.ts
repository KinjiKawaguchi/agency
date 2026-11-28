// What one feature of the Client API is to the mock node: the capability the node says it has, the options that switch it on, the routes it answers,
// and whether a hosted member is refused them. The mock node holds a list of these and asks each in turn, so that adding a feature is one new module
// and one line in `features/index.ts`, and no shared file has to say anything of it.

/** The settings of the running node that a feature may depend on, as the node has worked them out from the query and the environment. */
export interface ResolvedSettings {
  /** The mock evaluation numbers are on (`evaluation=1` or `AGENCY_MOCK_EVALUATION=1`). */
  evaluation: boolean
}

/** What a feature needs to answer with: the time the node says it is, which is pinned in tests. */
export interface FeatureContext {
  now: Date
}

/** The answer of a feature that the node has: a request for one of its routes is answered, and a request for any other is left to the node (nothing). */
export type Answer = (request: Request, url: URL, context: FeatureContext) => Response | undefined

export interface MockFeature<Options extends object = object, Asked extends object = object> {
  /** The name the node lists in the `capabilities` of `GET /api/about` when it has the feature, as docs/compatibility.md names it. */
  capability: string
  /**
   * What the node has of the feature, by the options it was made with: nothing when it has not, and when it has, the feature's answer, with its options
   * already taken in. This is the one place the options are looked at: the answer given back never looks at them again.
   */
  select: (options: Options) => Answer | undefined
  /** Whether the path is one a hosted member is refused: the owner's. */
  isOwnerRoute: (pathname: string) => boolean
  /**
   * How a running mock node (`vite preview`) is asked for the feature: the options it takes, read from the query of `POST /__mock/reset`.
   * Nothing for a query that does not ask for it. `resolved` holds the settings the running node has already worked out (a setting may come from the query
   * or from the environment), so that a feature never reads the environment itself.
   */
  fromQuery: (query: URLSearchParams | undefined, resolved: ResolvedSettings) => Partial<Options>
  /** What an end-to-end test asks of the feature, as the query that `fromQuery` reads: the other half of it, kept in the same module so that the two agree. */
  toQuery: (asked: Asked) => Array<[string, string]>
}

export const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

export const failure = (status: number, message: string): Response => json({ error: message }, status)
