# Versions and compatibility

Agency is made of parts that are shipped separately and upgraded at different times: nodes on different machines, team runtimes (harnesses), and apps on phones. This document says what has a version, what each part may change without breaking the others, how a breaking change is rolled out, and how long the old version stays. [releasing.md](releasing.md) says how releases are made; its numbering follows this policy.

## What has a version

Two kinds of things are versioned, separately.

**Artifacts**, the things that are shipped, each with its own semver (`MAJOR.MINOR.PATCH`):

| Artifact | Version and tag | What it includes |
|---|---|---|
| The node | `vX.Y.Z` in this repository | The node, the web client it embeds (also installed as an app on Windows and Linux, which therefore has no version of its own), and the CLI. They always ship together, so they always match. |
| The Apple apps | `apple-vX.Y.Z` in this repository: one marketing version for the iPhone and Mac apps, with a build number per platform | The native apps. An Android app would be `android-vX.Y.Z`. |
| scrum-harness | `vX.Y.Z` in its own repository | The harness and its runtime command. |

The crates (`agency-proto`, `agency-core`, …) have no versions of their own. They get one only if they are ever published for others to use.

**Contracts**, the agreements between artifacts, each with an integer version that changes rarely:

| Contract | Version as written | Between |
|---|---|---|
| Federation | `/fed/vN/…` | node and node |
| Signed text | `agency/vN/<kind>` | anyone who signs or checks a document: nodes, apps, the web client, the CLI |
| Team runtime | `agency-runtime/N` | node and harness |
| Client API | `clientApi: N`, served by discovery (below) | node and apps |

Each artifact states which contract versions it speaks, and a node says so through discovery, so an app and another node can check before relying on something.

## The rules every boundary follows

- **Additive by default.** A change that an older peer can safely ignore or refuse needs no new contract version. It does need a rollout rule when an older peer *refuses* it (see "Signed data").
- **Readers ignore what they do not know**, except where ignoring would be unsafe. A signed org definition is the exception: a node refuses a version with a field it does not know, because it cannot check what it cannot read (see "Signed data").
- **A breaking change gets a new contract version, side by side with the old one.** Both are served during an overlap. The old one goes only when the rule for that boundary says nobody needs it any more, and never in the same release that adds the new one.
- **A signature never stops being valid because the code changed.** Old documents keep their canonical JSON: new fields are left out when they are empty or absent. `testdata/vectors.json` pins every signed kind, and a test fails if an existing vector changes.

## Node and node (`/fed/v1`)

May change without a new version:
- A new route. An older node answers 404, so a caller treats 404 from an older node as "not supported", never as "not found".
  - A feature that has a capability (the table under "Discovery") is checked before the route is called: when what the caller kept of the node's `about` is no older than 3 days (the age the root's check of signed fields counts) and does not list the capability, the route is not called, and the reason says that the node runs release X, does not have the feature, and needs an update. The reads of a remote team's `activity` and `effort` do this. With nothing kept, or nothing recent, the route is called as before, and a 404 from a node that lists the capability stays "not found" (the team is not there).
- A new optional field in an answer. Readers ignore unknown fields in answers (TeamView, TokenUsage, and the rest).
- Stricter limits on what a node keeps from others, as long as no current node sends beyond them.

Breaking, so a new `/fed/v2`:
- removing or renaming a route or a field, or changing its type or meaning;
- changing what a signed read covers, other than by a new signed-text version of the read (see "Signed data"; the routes stay as they are);
- refusing something current nodes send.

Rollout of `/fed/v2`:
1. A node release serves both `/fed/v1` and `/fed/v2`, and says so in discovery.
2. A node uses v2 toward a peer only once that peer's discovery lists it.
3. A node release drops v1 only when both of these hold:
   - it has been 90 days since the release that added v2;
   - every node of every org the node belongs to lists v2.

   The node checks the second condition and, until it holds, says on its terminal which peers still speak only v1.

## Signed data

The org definition, decision envelopes, device changes, join requests, and signed reads are signed over `agency/v1/<kind>\n` and canonical JSON.

May change without a new version:
- **A new optional field in a signed body**, left out when absent, so existing documents keep their canonical JSON and signatures. Examples are `plan`, `aims`, and `kpis`.
- **The rollout rule for such a field.** Signed definitions are read strictly, so an older node refuses a version that carries a new field. The writer must therefore not write the field until every node that reads the document understands it. Each such field has a capability name in discovery (for example `orgPlan`, `orgKpis`):
  - the root's node checks that every node of the org lists the capability before it signs a version that uses it, and refuses otherwise, naming the nodes that are behind;
  - The check is made on every version the root signs, before it is signed, from what the node has kept of its peers (see "Discovery"), so signing never waits for the network. The nodes it checks are those of the **new** version's teams that do not run on the root's node, and it checks, for a node already in the org, the capabilities the new version uses and the current one does not use yet: a node that cannot read a field the current version already carries refuses the current version already, and refusing every later one would not help it. A node the version **brings into** the org (a first team on it, or a team moved to it) is checked against everything the version uses, since it would refuse every version from the start. The root cannot have read such a node yet, since only nodes of the org's teams are read, so `agency team-add --node` and `agency team-move` read it first; that works because a node that joined the org holds its definition and can answer. Taking a lagging node's teams out of the org leaves it out of the check.
  - A node counts as listing nothing when it was never read, never answered, or its last answer is more than **3 days** old. The refusal names each node, says why (not read, not answered, too old, or the capability it lacks and the release it runs), and says to update those nodes and run `agency sync`, which reads them again.
  - So one team node that never lists a capability holds back every version that would use the field, for as long as it runs teams of the org: the root signs no such version over it. The owner's remedy is to move that node's teams to another node (`agency team-move`) or to take them out of the org; both take it out of the check. There is no option to sign over a node that lags, because that would cut the node off from the org without anyone being told.
  - a client checks the same before it signs something with a new field.
  - The root can check only the nodes it can name: the nodes of the org's teams, which are the ones it sends each version to. A member's home node that hosts no team is not named in the definition, so it is not checked, and it refuses a version it cannot read until it is updated.

Breaking, so a new signed-text version `agency/v2/<kind>`:
- any change to canonical JSON;
- a change to the signed text's layout;
- a change to the meaning of an existing field.

Rollout: nodes first accept both `v1` and `v2` signatures, then start writing `v2` once every reader lists it, as with `/fed/v2`.

A new layout of one kind can come alone, and then it is announced by a capability (the first is `readAudience`, for `agency/v2/read`, a signed read that names the node it is for) instead of by `signedText`, which lists a version only when every kind has it. The rules for it are these:
- The new layout has its own kind line, and a header that travels with the signed document says which text was signed (for a read, `x-agency-audience` present means `agency/v2/read`). A verifier then makes one check, and neither text can pass for the other.
- A writer uses the new layout toward a peer only when the peer lists the capability, and the older layout otherwise.
- A verifier takes the older layout through the overlap. It may stop earlier: for an account that has already used the new layout (the account's node speaks it, so an older signature of that account is a copy), and for every account when its owner sets so. A way to take an account's older layout again is named in the refusal's log line, for an account that has two nodes of different releases. A document signed with `v1` stays valid for as long as it is kept: old org versions are never re-signed. New vectors are added for `v2`, and the `v1` vectors stay.

## Node and team runtime (`agency-runtime/1`)

The runtime lists what it speaks in `describe` (`runtimes`, `ops`), and the node checks that list before every other call.

May change without a new version:
- **A new optional op.** The runtime lists it in `ops`; the node calls it only then, and treats its absence as "not reported" (for example `tokenUsage`, `activity`).
- **A new optional field**, in a node's input (for example `byParticipant` in `decide`) or in a runtime's answer. Both sides ignore unknown fields.
- **A new value of an existing enum.** It is breaking for a reader that does not know it. The node therefore maps an unknown value to a safe default where one exists, and otherwise refuses the view (503) without crashing. The runtime adds such a value only after the node release that knows it.

Breaking, so `agency-runtime/2`:
- changing an op's input or answer;
- removing an op;
- changing an error status's meaning.

Rollout: the node speaks both versions and uses the highest one the runtime lists. A runtime may list both while nodes catch up. The node drops `/1` no sooner than 90 days after the node release that speaks `/2`, and only once the harness release that speaks `/2` is the oldest one in use.

## Node and apps (the Client API, `clientApi`)

The web client is embedded in the node, so it always matches its node and needs no rule here. The apps are shipped separately and drift.

May change without a new version:
- **A new route.**
- **A new optional field in an answer.** Apps must ignore fields they do not know.
- **A new optional field in an input.** The node must treat its absence as before.
- **A new enum value in an answer.** Apps must decode unknown values to a fallback ("other") and never fail the whole answer. Until every supported app does, a new value is breaking and waits.

Breaking, so `clientApi` goes up:
- removing or renaming a field or route;
- changing a field's type or meaning;
- making an optional input required;
- refusing inputs current apps send.

Rollout:
1. The node release that raises `clientApi` to N still serves N-1: the same routes keep their N-1 behavior unless the app says it speaks N (an `x-agency-client-api` header).
2. Discovery gives the node's `clientApi` range (`current`, `oldest`) and the oldest app version per platform that it serves.
3. On start, the app compares that with its own `clientApi`. If the node is too old, the app says plainly to update the node; if the app is too old, it says to update the app. Neither shows a broken screen.
4. The node drops N-1 no sooner than 90 days after the release that added N, and only once the oldest app build still in testers' and users' hands speaks N.

## Node data and configuration

- `config.json` and the data directory are read by one node only. Upgrades are forward-only: a newer node reads what an older one wrote, and migrates it on start if it must. A downgrade is not supported. A newer configuration may be refused by an older node, because the configuration is read strictly so that a typo never passes silently.
- A new configuration field is optional and has a default, so an existing `config.json` keeps working unchanged.
- The release notes name every migration. A migration keeps what it replaced, in the data directory under a dated name, so a failed upgrade can be undone by hand.

## Release numbers

The node's version moves by the Conventional Commits of each release ([releasing.md](releasing.md)).

- Before 1.0, a minor release (0.2 to 0.3) may contain breaking changes. A patch release never does, at any boundary.
- A breaking change is marked as one (`!` or a `BREAKING CHANGE:` footer) and says which boundary it breaks. Because contracts change side by side, a node release that only adds a contract version is not breaking: nobody has to move at once.
- From 1.0, a major version is needed only when a release drops a contract version, or migrates data in a way it cannot undo.

## Discovery

Each node says what it is and what it speaks, in two places.

- **For apps:** `GET /api/about`, admitted like any Client API read. It answers:
  - `node`: the node's version;
  - `contracts`: the versions spoken of `federation`, `signedText`, and `runtime`, and `clientApi` as `{ current, oldest }`;
  - `capabilities`: the names of optional features and signed fields (the table below), as strings, so that a reader ignores a name it does not know;
  - `oldestApp`: per platform (`ios`, `macos`, and later `android`), the oldest app version it serves. It is absent while the node sets no floor, which is until a breaking Client API change needs one. The node learns which app versions are in use from the `x-agency-client` header of the requests it admits (see the Client API in protocol.md).
- **For nodes:** `GET /fed/v1/orgs/<org>/about`, the same answer without `oldestApp`. It is a member's signed read of an org, like every other federation read (the signature is checked against that org's members, which is why the path names the org), so a stranger cannot learn a node's version and match it against known flaws. A node reads it from each node of an org (the nodes of the org's teams that do not run on it) in the loop that re-reads the replicas, and keeps it for that org and that node, not inside a replica, since a node hosts several teams. It reads it again when the kept answer is a day old, ten minutes after a read that failed, and ten minutes after a call to the node answered 404 (the node may not be the release it was read as). A read that fails keeps the last good answer and says why it failed; an answer beyond the limits of an `About` is not kept. `agency status` lists what a node has read of each peer, and `agency sync` reads them again.

### Capabilities

Names start at what came after the first release: every node has what the first release had (for example departments, devices, and P-256 keys). A name is one of two kinds: a **signed field**, which an older node refuses in a definition, so the root checks it before writing it; or a **feature**, which a client or a peer checks before relying on it instead of trying a route and reading a 404. A test keeps this table the same as the list in `agency-proto`, and another fails when an optional field is added to the org definition without a name here.

| Name | Kind | What it is |
|---|---|---|
| `orgPlan` | signed field | `teams[].plan` in the org definition: what the org intends for a team. |
| `orgAims` | signed field | `teams[].plan.aims`: the epics a plan aims at a KPI. |
| `orgKpis` | signed field | `kpis`: what the org measures. |
| `orgRecovery` | signed field | `root.recovery` and `root.pending`: the org's recovery key, and a recovery that was started and waits. |
| `orgSeats` | signed field | `seats`: places in the org that no one has taken yet (see Seat in the glossary), and the seats a member took: `people[].formerly` and `people[].askAbout`. |
| `orgExternals` | signed field | `externals`: people the org works with who never take a seat. |
| `hostedMembers` | signed field | `people[].hostedBy`: a member with no node of its own, hosted by the root's node, with no `publicKey`. |
| `webPush` | feature | A browser's Web Push subscriptions, and the notices sent to them. |
| `activity` | feature | A team's activity feed. |
| `outcomes` | feature | The owner's numbers and the readings of them. |
| `pushDevice` | feature | `device` in a push registration (an APNs token or a Web Push subscription), and a registration dropped with the device it names. |
| `effort` | feature | Where a team's tokens went, by work type: `GET /api/orgs/<org>/effort` and `GET /fed/v1/teams/<org>/<team>/effort`. |
| `readAudience` | feature | A signed read that names the node it is made for: the `x-agency-audience` header and the `agency/v2/read` signed text (see "Signed reads" in protocol.md). |
| `budget` | feature | A team's weekly budget status: `GET /api/orgs/<org>/teams/<team>/budget`. |

## Checks in CI

- **Vectors:** `testdata/vectors.json` must stay byte-identical for every existing entry, so signatures and canonical JSON never drift.
- **Fixtures:** `testdata/client/` must equal what the node answers.
- **Earlier versions:** the fixtures and vectors of each released version are kept, and today's code must still decode them (an app's decoding tests, and a node's reading of older org versions and replicas).
- **Bindings:** `bindings/` must equal what the Rust types generate.
- **Unknown words (Apple apps):** every enum the Swift package decodes reads a word it does not know as its own `unknown` (`OpenWireEnum`, or a decoder of its own with an `unknown` case), so that a new value never fails the whole answer and is never taken for a known one. `OpenWireEnumsGuardTests` reads the sources and fails for a decoded enum that does not, unless it is named there with why it is not a word of a node's (what the app keeps on the device for itself).
