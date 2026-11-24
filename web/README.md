# Agency web client

A single-page app for answering the requests your teams are waiting on. Vite, React, TanStack Router, TanStack Query, TypeScript.

```sh
npm install
npm run dev      # http://localhost:5173, served against a mock node
npm test         # component and unit tests
npm run build    # type-check and production build
```

## The mock node

`src/mock/node.ts` implements the Client API in memory from `testdata/teamview.sample.json`. The dev and preview servers mount it at `/api` (`vite/mock-node-plugin.ts`), and the tests call it through an injected `fetch`. Sample timestamps are shifted to the moment the node starts, so the desk always reads "30 minutes ago".

## Types and data

- TeamView types come from `../bindings`, generated from the Rust types. `src/api/teamview.ts` is the only module that imports them or the sample fixture.
- The Client API (`src/api/clientApi.ts`) is the web client's own side of the calls to its home node and is not generated yet.

## Language and theme

All user-visible text is in `src/i18n/en.ts`; `ja.ts` implements the same `Messages` type, so a missing key fails type-checking. Add a locale by adding a catalog and listing it in `src/i18n/locale.ts`. The locale comes from `?lang=`, then the stored choice, then the browser.

Colors are tokens in `src/theme/tokens.css`, each a `light-dark()` pair. The theme comes from `?theme=light|dark`, then the stored choice, then the system.

## Layout

Files are grouped by feature: `api/`, `desk/`, `people/`, `markdown/`, `i18n/`, `theme/`, `mock/`, `app/`.
