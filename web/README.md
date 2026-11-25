# Agency web client

A single-page app for answering the requests your teams are waiting on. Vite, React, TanStack Router, TanStack Query, TypeScript.

```sh
npm install
npm run dev      # http://localhost:5173, served against a mock node
npm test         # component and unit tests
npm run build    # type-check and production build
npm run e2e      # build, then drive the built client in headless Chromium against the mock node
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
- The Client API types (`Me`, `TeamSnapshot`, `DecisionInput`, `DecisionResult`, `Funding`, `OrgDefinition`, ...) also come from `../bindings`, through `src/api/clientApi.ts`, which is the only other module that imports them. Errors from the node are `{ "error": text }` with the status: 404 for something that is gone or not served, 409 for a refusal by the team's node, and a runtime's own 400 and 404 pass through.
- Token use is served by the node at `/api/orgs/<org>/teams/<team>/usage` (404 when the team's runtime does not report it, 503 when the team's node or runtime could not answer). The mock node serves it only when asked: `AGENCY_MOCK_USAGE=1 npm run dev`. Without it the mock answers 404 and the costs screen says the team does not report token usage.
- Funding forecasts (`src/costs/forecast.ts`) are worked out in the client from each window's use, reset time, and `lengthHours`.

## Language and theme

All user-visible text is in `src/i18n/en.ts`; `ja.ts` implements the same `Messages` type, so a missing key fails type-checking. Add a locale by adding a catalog and listing it in `src/i18n/locale.ts`. The locale comes from `?lang=`, then the stored choice, then the browser.

Colors are tokens in `src/theme/tokens.css`, each a `light-dark()` pair. The theme comes from `?theme=light|dark`, then the stored choice, then the system.

## Layout

Files are grouped by feature: `api/`, `desk/`, `people/`, `markdown/`, `i18n/`, `theme/`, `mock/`, `app/`.

## End-to-end tests

`e2e/` runs the built client (`vite preview` with the mock node) in headless Chromium with Playwright. It covers the decision flow, the other screens, language and theme, and takes a screenshot of every screen in light and dark at desktop and phone width into `e2e-screenshots/`. CI runs it on every pull request and keeps the screenshots as the `web-screenshots` artifact.

- The preview server sends the same security headers as the node (a Content-Security-Policy with `script-src 'self'` and `style-src 'self'`), so a change that needs an inline script or style fails here instead of in the owner's browser.
- The mock node keeps its state in memory, so each test starts it over with `POST /__mock/reset` (options: `usage=1`, `siteError=<reason>`, `outcome=queued`; the decision log starts with four earlier decisions, one in each state).
- Locally, `npx playwright install chromium-headless-shell` fetches the browser. If a headless Chromium is already on the machine, set `AGENCY_CHROMIUM_PATH` to its executable instead. No browser window opens either way.
- Behind Cloudflare Access an expired session redirects every call to a login on another origin, which the browser fails as a network error. A failed call from a page that is not on loopback (and while the browser is online) is taken for that: the page shows "Your sign-in expired" with a reload button, and drafts are kept in the tab's session storage across the reload. `POST /__mock/sign-in?expired=1` makes the mock answer that redirect; the end-to-end tests reach the preview server as `agency.localhost` for it.
- Device keys (agency#19): `src/device/` holds the page's Ed25519 key. It is generated non-extractable in WebCrypto and kept in IndexedDB, so the page can sign but cannot read the key out. `canonical.ts` writes canonical JSON exactly as `agency-proto` does, and the tests reproduce the bytes and signatures of `testdata/vectors.json`. The devices screen (the viewer's icon, top right) asks the node to add this device (`POST /api/devices/requests`), shows its pairing code while it waits, and on a device that is already added lists the requests with their codes and signs the change that adds one (`POST /api/devices/changes`). A decision is built as an envelope from the request's `answerWith`, signed with the key (signer `d-<12 hex>`), and sent to `POST /api/orgs/<org>/teams/<team>/envelopes`. A device that has no key, or is not added to the team's org, cannot decide: the desk says "Add this device to decide", links to the devices screen, and keeps what was written. There is no route where the node signs for the owner. The mock node verifies these signatures; `POST /__mock/device-add?code=...` stands in for `agency device-add` on the node's terminal.
- Inside the iPhone app (`apple/`, docs/apple.md): the app sets `window.agencyNative = { platform: "ios", push: null | { token, environment, label } }` before the page runs, and sets it again and dispatches `agency:native-push` when it changes. `src/native/` reads it, checks the shape the node takes (hex token of 32 to 200 characters, `sandbox` or `production`, a label), and registers it with `POST /api/push/tokens`. One token is sent once per page; a failed call is tried again on the next announcement or load. The token is never stored or logged by the page. Outside the app nothing is read or sent. The mock node keeps one entry per token and lists them at `GET /__mock/push-tokens`.
- Whose a request is: the node decides (`TeamSnapshot.mine`, docs/protocol.md "Requests") and the desk, the tab's count, and the app icon's badge (`window.agencyNative.setBadge`, only inside the iPhone app, only once every team has been read) all read that one list. The client never works it out from `to`.
- Who waits on the viewer is the node's call too (`TeamSnapshot.waitingOnViewer`, docs/protocol.md "People"): people show as needing the viewer only when it lists them, and a team that does not address the viewer has none. The client keeps no guess about the viewer's name.
