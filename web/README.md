# Agency web client

A single-page app for answering the requests your teams are waiting on. Vite, React, TanStack Router, TanStack Query, TypeScript.

```sh
npm install
npm run dev      # http://localhost:5173, served against a mock node
npm test         # component and unit tests
npm run build    # type-check and production build
```

## Screens

- **Decisions** (`/desk`): what the teams wait on you for.
- **People and work** (`/people`): one card per person, work in progress and blocked work, epic forecasts.
- **Org chart** (`/org`, `?unit=<department>` focuses on one department): departments, teams, people, and the agents of each team.
- **Costs** (`/costs`): each subscription window projected to its reset from the pace so far, and token use by day and by person.

## The mock node

`src/mock/node.ts` implements the Client API in memory from `testdata/teamview.sample.json` and `src/mock/fixtures.ts`. The dev and preview servers mount it at `/api` (`vite/mock-node-plugin.ts`), and the tests call it through an injected `fetch`. Sample timestamps are shifted to the moment the node starts, so the desk always reads "30 minutes ago".

Writes must carry the page's token: the served `index.html` has `<meta name="agency-token" content="__AGENCY_TOKEN__">`, the home node fills the placeholder in, and the client sends the value as `x-agency-token`. The dev and preview servers fill in the mock node's token.

## Types and data

- TeamView types come from `../bindings`, generated from the Rust types. `src/api/teamview.ts` is the only module that imports them or the sample fixture.
- The Client API (`src/api/clientApi.ts`: the signed-in person and funding, a team snapshot, the org definition, a decision) follows what the node will serve and is replaced by generated bindings when they exist. Token use is served by the mock node only.
- Funding forecasts (`src/costs/forecast.ts`) are worked out in the client from each window's use and reset time.

## Language and theme

All user-visible text is in `src/i18n/en.ts`; `ja.ts` implements the same `Messages` type, so a missing key fails type-checking. Add a locale by adding a catalog and listing it in `src/i18n/locale.ts`. The locale comes from `?lang=`, then the stored choice, then the browser.

Colors are tokens in `src/theme/tokens.css`, each a `light-dark()` pair. The theme comes from `?theme=light|dark`, then the stored choice, then the system.

## Layout

Files are grouped by feature: `api/`, `desk/`, `people/`, `markdown/`, `i18n/`, `theme/`, `mock/`, `app/`.
