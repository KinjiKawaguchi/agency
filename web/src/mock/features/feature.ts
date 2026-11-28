// What one feature of the Client API is to the mock node: the capability the node says it has, the options that switch it on, the routes it answers,
// and whether a hosted member is refused them. The mock node holds a list of these and asks each in turn, so that adding a feature is one new module
// and one line in `features/index.ts`, and no shared file has to say anything of it.

/** What a feature needs to answer with: the time the node says it is, which is pinned in tests. */
export interface FeatureContext {
  now: Date
}

export interface MockFeature<Options extends object = object, Asked extends object = object> {
  /** The name the node lists in the `capabilities` of `GET /api/about` when it has the feature, as docs/compatibility.md names it. */
  capability: string
  /** Whether the node has the feature, by the options it was made with. */
  isOn: (options: Options) => boolean
  /** The answer to a request for one of the feature's routes, or nothing when the request is none of them. It is asked only when the feature is on. */
  answer: (request: Request, url: URL, options: Options, context: FeatureContext) => Response | undefined
  /** Whether the path is one a hosted member is refused: the owner's. */
  isOwnerRoute: (pathname: string) => boolean
  /**
   * How a running mock node (`vite preview`) is asked for the feature: the options it takes, read from the query of `POST /__mock/reset`.
   * Nothing for a query that does not ask for it.
   */
  fromQuery: (query: URLSearchParams | undefined) => Partial<Options>
  /** What an end-to-end test asks of the feature, as the query that `fromQuery` reads: the other half of it, kept in the same module so that the two agree. */
  toQuery: (asked: Asked) => Array<[string, string]>
}

export const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

export const failure = (status: number, message: string): Response => json({ error: message }, status)
