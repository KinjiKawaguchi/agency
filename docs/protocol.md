# Agency protocol between nodes (v1)

Agency runs organizations in which AI agents and people work together, across many machines.
There is no central service.
The only central thing is authority (the key of the person who decides for the org); the data is split across machines.

## Units and writers

| Kind | Unit | Writer | Where it lives |
|---|---|---|---|
| Execution (activity, requests, decisions, board) | Team (one harness) | The node the harness runs on | The team's node |
| Governance (departments, members, where teams run, revoked keys) | Org | The org's root key (the person who decides) | Copied to the nodes of the org's teams and of its members |
| Account (key, memberships) | Account | That account's human | Its home node |

Every piece of data has exactly one writer.
Other nodes only keep replicas and never change them.
To change something, send a signed envelope to its writer.

## Identifiers

- Ids of accounts, orgs, teams, departments, and members: `[a-z0-9][a-z0-9-]{0,39}`
- Envelope id: 16 random bytes as hex (32 characters)
- Time: RFC 3339 in UTC (`2026-11-24T12:00:00Z`)

## Keys and signatures

- Root and account keys are Ed25519; a definition that lists any other kind is refused. A device key is Ed25519 or P-256, so a phone can keep it in hardware that only does P-256.
- An Ed25519 public key is written as its 32 bytes in standard base64 with `=` padding. A P-256 public key is written as `p256:` and its 65-byte uncompressed point (`0x04`, x, y) in standard base64 with `=` padding. No other form is read (not the compressed point), so a key has one text, one fingerprint, and one device id.
- An Ed25519 signature is its 64 bytes in standard base64. A P-256 signature is ECDSA with SHA-256 over the signed text, written as 64 bytes (`r` then `s`, 32 bytes each) in standard base64; either `s` is accepted (replay protection keys on the envelope id, not on the signature).
- Every node must understand P-256 keys before a person adds a P-256 device: a node that does not refuses the org versions that list one.
- Three kinds of key, each limited to what it may sign:
  - The root key, held by the root's node, signs org definition versions.
  - An account key, held by the account's home node, signs reads, join requests, and the account's first device in an org. It never signs a decision.
  - A device key, held by a client (a browser, a phone, or the CLI) and never by a node, signs decisions and changes to its member's devices.
- Fingerprint: the first 16 bytes of the SHA-256 of the public key's bytes (the 32 bytes of an Ed25519 key, the 65-byte point of a P-256 key, never the text), as hex (32 characters). Invite links carry it, so the invited node can check that the org's root key is genuine.
- The signed text is a kind line followed by canonical JSON: `agency/v1/<kind>\n` and then the body's canonical JSON. The kinds are `envelope`, `org`, `join`, `read`, and `device`. Because of the kind line, a signature for one kind cannot be reused for another.
- Canonical JSON: object keys sorted by bytes, no whitespace, strings escaped as standard JSON, integers only, and fields whose value is null left out.
- A signed document: `{ "body": <body>, "signer": <id>, "signature": <base64> }`. `signer` is an account id, or, for a document signed with a device key, that device's id.

## Org definition (kind `org`)

```json
{
  "org": "acme", "label": "Acme", "version": 3, "prev": "<fingerprint of the previous version>",
  "root": { "account": "alice", "publicKey": "..." },
  "departments": [{ "id": "dev", "label": "Development", "parent": null, "head": "owner" }],
  "people": [{ "id": "owner", "label": "Alice", "account": "alice", "publicKey": "...", "department": "dev", "role": "member",
              "devices": [{ "id": "laptop", "label": "Laptop", "publicKey": "...", "addedAt": "2026-11-24T12:00:00Z" }] }],
  "teams": [{ "id": "app", "label": "app", "department": "dev", "node": "https://alice.example" }],
  "revoked": ["<revoked public key>"],
  "at": "2026-11-24T12:00:00Z"
}
```

- Only the root key (`root`) can sign it. `signer` is `root.account`.
- Versions start at 1 and grow by 1. `prev` is the hex SHA-256 (64 characters) of the canonical JSON of the previous signed version. Version 1 has no `prev`.
- A node accepts only the next version of the one it has: the version is one higher, `prev` is the current version's fingerprint, and it is signed by the current root key. A new root is also signed by the previous root key.
- A node that is more than one version behind (it was offline, or reads rarely) takes the versions in between first, in order, from the node it reads the definition from (`GET /fed/v1/orgs/<org>/versions?after=<n>`), checking each as above. It does so when a read of the definition shows a newer version, and when the root's node answers a device change with one. Every node keeps the versions it accepted, so the root's node can serve all of them.
- Membership: an account listed in `people` whose `publicKey` is not in `revoked`.
- `devices` is left out when it is empty, so versions signed before devices existed keep their canonical JSON and signatures. Device ids are unique per member. A device key appears once in the whole definition, is never an account key or the root key (nodes hold those), and is never in `revoked`.
- Departments form a tree: a department's `parent` is another department of the org (or absent at the top), following parents never leads back to the department, and `head` is a member's `id`. A member's and a team's `department` is one of the org's departments. The root's node changes them with `agency dept-add`, `dept-set`, `dept-remove`, and `team-dept` (and `team-add --department`), each a new version.
- A team's `label` is changed with `agency team-set`, and a team leaves the definition with `agency team-remove`, each a new version written by the root's node. A node refuses `team-remove` for a team that is still in its own configuration: it would be left running outside its org.
- A node serves a team only while the team is in its own configuration **and** in the org's definition it holds. After the version that removes a team, its node answers reads and envelopes for it like for a team it does not run, until its owner takes it out of the configuration (`agency status` says so, and so does the start of `serve`). That version is queued for the removed team's node as well as for the nodes of the teams that remain, and a node that takes it deletes its replica of the removed team's view.
- `teams[].node` is the origin where other nodes reach a team. The node that runs a team knows it from its own configuration, not from this field, so a team keeps running and being served if the field still names an old origin of its node. That node reports the mismatch. The root's node fixes it with a new version that changes only that team's `node` (`agency team-move`).
- `role`: `member` (reads and answers) or `answerer` (only answers). Either decides only the requests addressed to them (see "Requests"). An answerer also reads only those: a team's node serves an answerer a TeamView with just the requests addressed to it, and no people, work items, or epics, and does not serve it token usage (403).

## Devices (kind `device`)

A device change adds or removes one of a member's devices:

```json
{ "org": "acme", "account": "bob", "op": "add",
  "device": { "id": "phone", "label": "Phone", "publicKey": "...", "addedAt": "2026-11-24T12:00:00Z" },
  "at": "2026-11-24T12:00:00Z" }
{ "org": "acme", "account": "bob", "op": "remove", "device": "phone", "at": "2026-11-24T12:00:00Z" }
```

- Who signs: while the member has no devices, only the account key, and only to add the first device. After that, only one of the member's devices (named by `signer`), never the account key.
- The account's home node checks the change against its copy of the definition and relays it to the org's root node with `POST /fed/v1/orgs/<org>/devices`. It knows where the root's node is from the invite it joined with, which only the root's node creates. If that node can't be reached, the change waits in the outbox. The root's node answers with the version it wrote, and the home node keeps it. The root node checks the signature, that `account` is a member, and that `at` is at most 7 days old and at most 5 minutes ahead, then applies the change and checks the result like any version. It writes the result as the next org definition version. Like every version, that one is signed by the root key and reaches the other nodes as usual.
- Removing a device also adds its key to `revoked`, so a lost device can never sign again, under any id. A removal takes effect on each node as soon as it holds the new version, including for envelopes still waiting in outboxes.
- Adding a device works like this:
  1. The new device asks its home node to add it, through the Client API. The node keeps the request for an hour, and at most 10 requests.
  2. The person compares pairing codes. For the first device, the node's terminal shows the code (`agency device-add`) and signs the change with the account key. No Client API route signs with the account key. For later devices, one of the member's devices lists the request and signs the change. That device can be a client, or the CLI device on the node's terminal once it has been added.
  3. A device's id is `d-` followed by its pairing code without dashes.
- Pairing: a new device shows its pairing code, the first 12 hex characters of its key's fingerprint in groups of 4 (`dbc2-9825-1c51`). The person checks that the device that signs the add shows the same code for the key it is adding. The code has 48 bits, so a node that relays the key cannot find another key with the same code while the person is pairing.

## Signed reads (kind `read`)

A node that reads a team on another node adds these headers:

- `x-agency-account`: the account id
- `x-agency-date`: the time, with milliseconds (`2026-11-24T12:00:00.123Z`), so two reads made in one second are signed differently
- `x-agency-signature`: the signature of `agency/v1/read\n` + `<method>\n<path>\n<time>\n<account>`

`<path>` is the request path without its query, except on a route that takes a query (`usage`): there it is the path with the query exactly as the route table writes it, built from the parsed values, so a signature covers the range it reads.

The receiving node answers only when the time is within 5 minutes either way, the signature matches the account's public key in the org definition, and the account is a member of the team's org.
It accepts each signature once: within the 5 minutes a signature is good for, a copy of it is refused.
A read that fails any of these checks is refused like a read of an org or team the node does not have (see "Federation routes").

## Decision envelope (kind `envelope`)

```json
{ "id": "<32 characters>", "kind": "answer", "org": "acme", "team": "app", "target": "r-20260101-000000-abcd",
  "payload": { "text": "", "choice": "a" }, "account": "alice", "at": "2026-11-24T12:00:00Z" }
```

- `kind`: `answer` (answers a request; `target` is the request id) or `judge` (decides a board item waiting for a decision; `target` is the item's reference). Either way `target` is 1 to 120 bytes of ASCII letters, digits, `.`, `_`, `#`, and `-`; anything else is refused as a malformed target (see "Item references").
- `signer` is the id of one of the deciding member's devices. The signature is checked with that device's key, found under the member whose `account` is the envelope's `account`. A signature by an account key is refused.
- The receiving node (the team's node) applies an envelope only after checking the signature, membership, expiry (7 days after it was made, and at most 5 minutes ahead), that the envelope's org and team match where it was sent, that its id has not been applied before, and that its account is the addressee of the request it decides. A repeated envelope is not applied again; the node answers that it received it, even if the request no longer waits.
- The request an envelope decides is the one in the team's current view whose `answerWith` is the envelope's `kind` and `target`. Its addressee is the member the rule in "Requests" gives (the same rule as `mine`). An envelope from anyone else, or for a request that is not waiting, is refused with 403, and both get the same refusal.
- The sending node keeps an envelope it could not deliver in its outbox and retries with backoff (15 s, 30 s, 1 min, 2 min, 5 min, then every 5 min). It does not retry when the receiver refuses with a 4xx (408 and 429 are retried).
- Nodes accept envelopes signed by a device of the deciding member. The CLI signs with its CLI device, and clients send device-signed envelopes through `POST /api/orgs/<org>/teams/<team>/envelopes`. The home node checks them as the team's node will, and never signs them itself.

## Invites and joining (kind `join`)

1. The root key's node creates an invite. Invite link: `agency:join?node=<node origin>&org=<org>&fp=<root key fingerprint>&code=<code>`. The code is 16 random bytes as base64url. The node keeps only the code's SHA-256. An invite expires after 7 days and can be used once.
2. The invited account's home node reads the org definition with `GET <node>/fed/v1/orgs/<org>/definition?code=<code>` and checks that the root key's fingerprint matches the link's `fp`. If it does not, it stops (the node may be fake).
   An org id names one org on a node. If the node already has an org with the link's id (a stored definition, or a trusted root fingerprint) under another root key, it refuses the link and changes nothing: joining would replace its definition and its trust for that id, and with them the teams it hosts there. A node also never creates an org with an id it already trusts.
3. When the invited person accepts, their node signs a join request and sends it to `POST <node>/fed/v1/orgs/<org>/join`. Body: `{ "org", "code", "account", "label", "publicKey", "at" }`. The signature is checked with the body's `publicKey`, and `at` must be within 10 minutes of the root node's clock.
4. The root key's node checks the code, its expiry, and that it is unused, signs the next version with the new member added at the place the invite set (department, role), and returns it to the requesting node. The new version goes to the nodes of the org's teams through the outbox.

## Federation routes

| Method | Path | Checked |
|---|---|---|
| GET | `/fed/v1/orgs/<org>/definition` | A member's signed read, or an open invite code |
| POST | `/fed/v1/orgs/<org>/definition` | It is the next version (signed by the root key) |
| GET | `/fed/v1/orgs/<org>/versions?after=<n>` | A member's signed read of the path with its query. Answers `{ "versions": [<signed definition>, ...] }`: the versions after `n`, oldest first, at most 20 (a node further behind asks again); 404 when this node does not have them all |
| POST | `/fed/v1/orgs/<org>/join` | The join request's signature and the invite |
| POST | `/fed/v1/orgs/<org>/devices` | Only on the root's node (403 elsewhere, once the change's signature checks out): a device change, checked as in "Devices". Answers the version it wrote |
| GET | `/fed/v1/teams/<org>/<team>/view` | A signed read and membership (answers `{ "view": TeamView, "viewer"?: <name> }`) |
| GET | `/fed/v1/teams/<org>/<team>/usage?days=<n>` | A signed read of the path with its query, and membership (answers `TokenUsage`; 400 for a range outside 1 to 90; 404 when the team's runtime does not report usage) |
| GET | `/fed/v1/teams/<org>/<team>/evaluation?days=<n>` and `/effort?days=<n>` | Like `usage`: a signed read of the path with its query, and membership as a member (an answerer gets 403). Answers `Evaluation` or `Effort` with that one team in `teams` or `unavailable`; 400 for a range outside 1 to 90 |
| POST | `/fed/v1/teams/<org>/<team>/envelopes` | The envelope's signature, membership, expiry, and that it was not applied before |

The federation routes bypass Cloudflare Access, so anyone can call them, and nothing in an answer may tell a caller what the node holds until the caller is authenticated.
Every request that fails before that gets `404` with the body `{"error":"not found"}`, whatever the reason:

- the org or the team is not on this node, or its id is malformed;
- a signed read is missing, malformed, more than 5 minutes off the node's clock, signed by a non-member, or signed by another key;
- an envelope names a non-member, a device the member does not have, or is not signed by that device's key;
- a device change names another org or a non-member, or is not signed by the key that "Devices" requires;
- an org definition is not signed by the root key the node knows for the org (the root of its copy, or, without a copy, the root whose fingerprint it trusts).

A node never follows a redirect in an answer to its own requests (to other nodes, to the Access certs, or to APNs): others choose where it sends, and a redirect would carry its signed requests elsewhere. A 3xx answer is a failed one.

The node writes the real reason for such a refusal to its own log, never to the caller: at most 20 lines a minute, then a count of the rest, except that a refusal of a request with a valid signature by a member (for example one whose clock is off) is always written.

Once the request is authenticated, refusals are specific (an expired envelope, a version that is not the next one, a node that does not hold the root key).
A request whose signature is well formed spends one signature check even when the org, team, member, or device is missing, so the time of an answer does not tell either (apart from reading the node's files).
The checks that do not depend on what the node holds come first and answer as they always do: a malformed body (400), an envelope whose org and team differ from the path (400), and a join request (whose invite code is its credential: 403 for an unknown, used, or expired code, also in an org the node does not have).

## TeamView

A TeamView is what a team runtime reports about its team: its participants (`people`), the requests waiting for a decision, its work items, and its epic forecasts.
The node reads it from the team runtime (`view` in agency-runtime/1), returns it from `GET /fed/v1/teams/<org>/<team>/view` as `{ "view": TeamView, "viewer"?: <runtime name> }`, and serves it to clients.
`viewer` is the signed reader's name in the team runtime (see "Requests" below for the mapping); it is absent when the team does not address the reader.
A node that reads another node's TeamView parses it into these types before keeping a replica, and drops it if it does not parse or is beyond the limits below. It reads at most 2 MiB of any answer from another node.

Limits, in characters for text: ids, labels, names, and refs 200; a request's or work item's `title` 200; `context` 4,000; `document` 40,000; a person's `status`, an option's `text`, and a forecast's `basis` 500; an option's `detail` and a thread message 4,000. At most 200 people, 200 requests, 5,000 work items, 500 epics, 20 options per request, 200 thread messages, and 100 entries in `holding`, `blocks`, or an epic's `blocked`. A node serves a runtime's view only within them too (`agency_proto::view::limits`).

The Rust types in `agency-proto` (`view.rs`, `runtime.rs`, `timestamp.rs`) are the source of truth.
The TypeScript types in `bindings/` are generated from them with ts-rs and are never edited by hand; `scripts/check-bindings.sh` regenerates them and fails on any difference.
`testdata/teamview.sample.json` is the shared sample: the Rust tests, the web client, and the harness all read it.

Conventions:

- Keys are camelCase. Numbers are non-negative integers.
- `Timestamp`: RFC 3339 in UTC, ending in `Z` (`2026-11-24T12:00:00Z`; fractional seconds are allowed). `Date`: a calendar date, `YYYY-MM-DD`. They are different types.
- An optional field is omitted when absent. It is never written as `null`.
- Participants and addressees are named by the team runtime's own names (for example a session name, or `owner`), not by Agency org member ids. The team runtime does not know org member ids, except the one the node gives it in a decision (`by`), which is why a thread message can carry it (below).
- Unknown fields are ignored, so a team runtime may add fields before the contract names them.

```
TeamView      { org: Id, team: Id, label, generatedAt: Timestamp,
                people: PersonStatus[], requests: Request[], work: WorkItem[], epics: EpicForecast[] }
PersonStatus  { id, label, kind: "agent" | "human", role: "lead" | "member",
                state: "working" | "idle" | "away" | "unknown", since?: Timestamp,
                waitingOn?: { kind: "person", person } | { kind: "lead" },
                status /* one line */, holding: string[] /* work item refs */ }
Request       { id, kind: "question" | "proposal" | "pendingDecision", title, context /* plain text */,
                document?: string /* Markdown, proposals */, from, to?, createdAt: Timestamp,
                issue?: string, blocks: string[], options: RequestOption[], thread?: ThreadMessage[],
                answerWith: { kind: "answer" | "judge", target } }
RequestOption { key, text, isRecommended: boolean, detail?: string }
ThreadMessage { from, text, choice?: string, by?: Id, at: Timestamp }
WorkItem      { ref /* item reference */, title, status: "backlog" | "next" | "inProgress" | "release" | "done",
                assignee?: string, blocked?: { reason: BlockedReason, since: Date }, epic?: string,
                points?: number, eta?: { p50: Timestamp, p80: Timestamp, basis, isOverBudget: boolean },
                workType?: WorkType, doneAt?: Timestamp }
WorkType      "newFeature" | "improvement" | "maintenance" | "debt" | "incident"
BlockedReason "needsDecision" | "external" | "waitingOnPrevious" | "timeOrEquipment"
EpicForecast  { ref, title, total, done, doing, remainingPoints, blocked: { reason: BlockedReason, count }[],
                forecast?: { days, at: Timestamp }, doneAt?: Timestamp, basis }
```

Item references:

- A work item is named by an **item reference** (`WorkItem.ref`, `EpicForecast.ref`, and where a work item is named: `WorkItem.epic`, `Request.issue`, `Request.blocks`, `PersonStatus.holding`). It is an opaque string that the team's harness chooses, for example `app#12` or `ENG-123`. Nothing in a node or a client splits it, reads a number out of it, or builds a link from it, and two references are the same only when the strings are equal. A view allows 200 characters.
- One use is stricter. The `target` of a `judge` envelope is an item reference, and an envelope's `target` is checked by the node (400, `malformed target`) and by the iPhone app before it signs: 1 to 120 bytes of ASCII letters, digits, `.`, `_`, `#`, and `-`. So a pending decision's `answerWith.target` has to follow that rule, even where `ref` and `issue` are longer or hold other characters (`owner/repo#12`, a URL).
- A runtime whose tracker's ids do not fit gives the pending decision a reference that does (for example `PROJ-12`, or a short hash), keeps a lookup from it to the item, and reads it back in `decide`. The item's `ref` and `issue` may stay as the tracker writes them. The rule is widened only when a real tracker needs it, because every node and the app must accept the wider rule before a harness sends it.

Participants (`people`, humans and agents):

- `state` says whether the participant is active; `waitingOn` says who has to act before they can continue. A participant can be `working` and still wait on someone, for example on a permission prompt: then `waitingOn` names the participant who answers the prompt and `status` says it is a permission prompt.
- `waitingOn.person` uses the same names as `Request.to`.
- Who waits on the reader is decided by the reader's home node and given to clients as `TeamSnapshot.waitingOnViewer`: the `id`s of the participants whose `waitingOn` is `person` with the reader's runtime name in that team (`viewer`). Unlike a request's `to`, a name that is not the reader's is never theirs, because `waitingOn` can name anyone on the team, agents included. A reader the team does not address (no `viewer`) has nobody waiting on them.

Work items:

- `workType` is the board's "Work type" field of the item (see "Work type" in the glossary), as the team's runtime reads it. It is left out when the board has none; a runtime never guesses it, and whoever needs a type for an item without one (the node, when it splits effort by work type) works it out from the item's pull request itself. A value outside the five names fails the view's parse, like any other malformed field.
- `doneAt` is when the item first reached `done`: the board's own time when the tracker has one, otherwise when the runtime first saw it (up to one refresh late). It is left out while the item is not done and when it is not known; an item that goes back and is done again keeps the first time. `EpicForecast.doneAt` is when the epic's last child reached `done`, present only once `done` equals `total`. A runtime may keep a finished epic in `epics` for a while so that this time can be read (the scrum harness keeps it 30 days): it has `done` equal to `total`, nothing `doing`, nothing remaining, and no `forecast`, and a client shows it as finished, after the open epics, and does not count it in what remains. `work` lists open items only, so `WorkItem.doneAt` appears only on an item still in the view. The node passes both on unchanged.

Requests:

- `question`: the team asks something. `proposal`: the team asks for approval of a document (`document`) with options. `pendingDecision`: a board item is blocked until someone decides.
- `answerWith` is exactly the `kind` and `target` of the decision envelope that answers the request, so clients never derive them. A `pendingDecision` is answered with `judge` and its item reference; the others with `answer` and the request id. So the `answerWith.target` of a pending decision follows the stricter rule of "Item references" below.
- `to` absent means the team's owner. The node maps runtime names to org members through its hosted-team configuration: `owner` is the org member id of the team's owner, and the optional `people` map names other runtime names (`{ "<runtime name>": "<org member id>" }`). A `to` that is not in `people` goes to the owner.
- In the other direction, an org member's runtime name is their key in `people`; otherwise `owner` if they are the team's owner; otherwise they have none in that team.
- Which requests are a reader's to decide ("mine") is one rule, computed by the reader's home node and given to clients as `TeamSnapshot.mine`. The desk, the badge counts, and push notifications all use it, and the team's node applies a decision only from the member it gives (see "Decision envelope").
  - In a team on that node, a request is the reader's when its addressee, as above, is the reader. So a request with no `to`, or with a `to` that is not in `people`, is the team owner's.
  - In a team on another node, the home node knows only the reader's runtime name there (`viewer`). A request is the reader's when its `to` is that name, or when it has no `to` and that name is `owner`. A reader who owns that team under another runtime name is not given the requests without `to`; this is a known limit until the team's node reports it.
  - A reader the team does not address (no `viewer`) has none.
- `context`, `detail`, `status`, and messages are plain text. `document` is Markdown; clients render a safe subset and never render raw HTML from it.
- `thread` holds earlier messages, oldest first, when a request was answered and then reopened.
  - `from` is the runtime's name of the participant the message is from. `by` is optional and, when a message carries a decision, names the org member who decided: an `Id`, the `by` of the decision the node passed to the runtime (see "agency-runtime/1"), never a participant name. A message the team wrote has no `by`. A `by` that is not a valid `Id` makes the whole view fail to parse, like any other malformed field.

## agency-runtime/1

The node does not read or write a team runtime's files. For each hosted team it runs a configured command (argv and environment) once per call:

1. It writes one JSON request to the command's stdin and closes stdin.
2. It reads one JSON response from stdout. Text in a decision travels only on stdin, never in arguments.
3. The command exits with 0 when the response is a result and 1 when it is an error. The node stops the command after 20 seconds and treats that as unreachable.

Request: `{ "runtime": "agency-runtime/1", "org": <Id>, "team": <Id>, "op": <op>, ...input }`.
Response: `{ "result": <result> }` or `{ "error": { "status": <HTTP status>, "message": <text> } }`.

| op | input | result |
|---|---|---|
| `describe` | none | `{ "runtimes": ["agency-runtime/1"], "ops": ["describe", "view", "decide"], "label": <text> }` |
| `view` | none | TeamView |
| `decide` | `"decision": { "kind": "answer" \| "judge", "target", "by": <org member id>, "byParticipant"?: <participant name>, "payload": { "text", "choice"? } }` | `{}` |
| `tokenUsage` (optional) | `"days": <1 to 90>` | `TokenUsage` (see "Token usage") |
| `activity` (optional) | `"since": <Timestamp>`, `"limit": <1 to 500>` | `Activity` (see "Activity") |

- The node calls `describe` first and stops unless `runtimes` contains `agency-runtime/1`. Unknown names in `ops` are ignored.
- It runs at most one `view` and one `tokenUsage` per team at a time. Callers that arrive meanwhile share the answer, and the node reuses usage for 60 seconds (for the same range).
- **A read waits for no runtime process once the node has an answer.** The node keeps each team's last view (and its activity window) in memory. A view younger than 5 seconds (activity: 10) is served as it is. An older one, up to 60 seconds old, is served at once and one refresh starts in the background. Only a read with nothing in memory, or nothing younger than 60 seconds (the first read of a team, or after the runtime has failed that long), waits for a run, and readers that must wait share the run already in flight. So a view or an activity read can be up to 60 seconds old; the runtime's own `generatedAt` says when it was made. A refresh that fails keeps the last good answer until it is that old, and then the failure is answered. The node counts the reads that were served young, served old, and had to wait, so cold reads can be told from warm ones.
- A decision goes to the runtime directly. It drops what is kept for the team and starts a refresh of its view at once, so the read that follows finds a fresh answer or joins the run in flight.
- The node calls `decide` only after it has verified the decision envelope (signature, membership, expiry, team, replay, and that the decider is the request's addressee). `by` is the org member who decided, and `byParticipant` the same decider as a participant of the team: their runtime name, in the same name space as a request's `to` (their key in the hosted team's `people`, otherwise `owner`). The node sends `byParticipant` with every decision; a runtime should refuse (404) a decision on a request whose `to` names another runtime name. The node maps a `to` it does not know to the owner, and the runtime does not know the map, so for such a request only a decision with `byParticipant` `owner` is the addressee's.
- Error statuses: 400 (malformed input), 404 (no such request or work item), 503 (the runtime cannot act now). The node treats 400 and 404 as refusals and does not retry them; it retries every other failure, including a timeout or output that is not a response.
- `tokenUsage` is optional. The node calls it only when `describe` lists it in `ops`; otherwise it answers 404 without running the command. A runtime's 404 also means it has no usage to report; any other failure is 503.
- `activity` is optional in the same way: the node calls it only when `describe` lists it, and otherwise answers 404 without running the command. It runs at most one `activity` call per team at a time, and the answer is kept and refreshed as above (young for 10 seconds): the command is asked for the events after five minutes before the first reader's `since`, at the largest `limit`, and each reader gets the events after its own `since`, cut at its own `limit`, so readers with different questions start one process. A reader whose question the shared answer cannot give (it asks from before the shared answer began, or for more events than an answer the runtime cut holds) gets a run of its own.
- `usage` is a reserved op name. Subscription usage belongs to the account, not to a team runtime, and is not part of agency-runtime/1.

## Token usage

```
TokenUsage  { org: Id, team: Id, days, generatedAt: Timestamp, byDay: DayCost[], byPerson: PersonUsage[] }
DayCost     { date: Date, costUsd: { <runtime name>: number } }
PersonUsage { person: <runtime name>, costUsd: number, tokens: { input, output, cacheRead, cacheWrite } }
```

- A report covers the last `days` calendar days, today included, in the team runtime's time zone. `byDay` is oldest first; days without use may be left out.
- Participants are named by their runtime names, as in TeamView, not by org member ids.
- `costUsd` is an estimate: tokens priced at API rates, in US dollars rounded to cents by the runtime, whatever actually pays for them (a subscription or an API key). It is the one exception to "numbers are non-negative integers": a non-negative JSON number. Token counts are non-negative integers.
- The node computes nothing and stores nothing; every read asks the team's runtime. Before passing a report on, the node checks that `org`, `team`, and `days` are the ones it asked for, that `byDay` has at most `days` entries with strictly increasing dates, that every cost is finite and non-negative, and that every participant has a name. A report that fails is 503.
- The shared sample is `testdata/tokenusage.sample.json`.

## Evaluation

How fast a team's work ships, and where its tokens go. The team's node works the numbers out (nothing is stored: each read asks GitHub and the team's runtime, and the node reuses an answer for 10 minutes), so a screen and an agent that cites them agree.

```
Evaluation       { org, days, from: Date, until: Date, generatedAt, teams: TeamEvaluation[], unavailable: TeamUnavailable[] }
TeamUnavailable  { team: Id, reason: string }
TeamEvaluation   { team, from: Date, until: Date, summary: EvaluationSummary, byDay: DayEvaluation[] }
EvaluationSummary{ merged: MergedCounts, prLeadMedianSeconds?, prLeadP90Seconds?, issueLeadMedianSeconds?, issueLeadP90Seconds?,
                   mainRedSeconds, longestRedSeconds, reverts, costPerMergedPrUsd? }     // ? = left out when there is no value
MergedCounts     { feat, fix, refactor, docs, test, chore, other }
DayEvaluation    { date, merged: MergedCounts, reverts, mainRedSeconds, costUsd, prLeadSeconds: int[], issueLeadSeconds: int[] }

Effort           { org, days, from, until, generatedAt, teams: TeamEffort[], unavailable: TeamUnavailable[] }
TeamEffort       { team, from, until, byType: ByWorkType<TypeUsage>, byDay: DayEffort[] }
DayEffort        { date, costUsd: ByWorkType<number> }
ByWorkType<T>    { newFeature: T, improvement: T, maintenance: T, debt: T, incident: T, unclassified: T }
TypeUsage        { costUsd: number, tokens: { input, output, cacheRead, cacheWrite } }
```

- The period is the last `days` calendar days (1 to 90), today included. `from` and `until` are its first and last day, written like `byDay[].date`, in the time zone of the node that computed them: each team has its own (the time zone of the team's node), and every day of its `byDay` lies inside them, so a client draws a team against its own period. The answer's own `from` and `until` are the answering node's. `byDay` is oldest first, and days with nothing in them may be left out.
- Counts and seconds are integers. Costs are estimates at API prices, as in "Token usage". An optional field is left out, never null.
- A pull request counts on the day it merged. Its type is the Conventional Commits type of its title (`feat`, `fix`, `refactor`, `docs`, `test`, `chore`); anything else, including `ci`, `build`, `perf`, `style`, is `other`.
- A **revert** is a merged pull request whose title starts with `Revert` or `revert`, or whose body has a line that starts with `This reverts commit <sha>` (what `git revert` writes) or `Reverts <owner/name>#<number>` (what GitHub's Revert button writes); a sentence that only mentions those words does not count. It is counted in `reverts` and not in `merged`, so the pull requests merged are `merged` and `reverts` together.
- **PR lead time** is opened to merged. **Issue lead time** is the issue's first move to In Progress on the board (else the pull request's first commit) to merged; a pull request with no closing issue has none.
- **Main red**: for each push to the default branch, the commit's workflow runs decide (all workflows). A commit is red when a run failed or timed out and green when all passed or were skipped; a run cancelled by a newer push counts for nothing. Red lasts from the completion of the first red commit to the completion of the next green one, or to `generatedAt` while it is red. A span across midnight is split between the days. `longestRedSeconds` is the longest single span.
- The summary's p90 is the nearest rank (`sorted[ceil(0.9 n) - 1]`); the median of an even count is the mean of the two middle samples, rounded down. A median or p90 is left out with no samples.
- `costPerMergedPrUsd` is the period's cost divided by the pull requests merged in it (`merged` and `reverts`). Token usage is per day, so there is no per-pull-request attribution. It is left out when nothing merged.
- **Work type** (`Effort`): the board's "Work type" field when the item has one; otherwise the pull request's type: `feat` is `newFeature`, `refactor` is `debt`, `fix` is `maintenance` (`incident` when its issue carries the label `incident`), and `docs`, `test`, `chore` are `maintenance`. Work with neither is `unclassified`. `improvement` comes only from the board field. The intended allocation and budget are the org definition's team plan; `Effort` does not copy them.
- A team whose answer cannot be had (not configured, its node unreachable, GitHub unavailable) is listed in `unavailable` with a reason in plain text, without a secret or an address, and the other teams are answered. A malformed answer from another node is such a failure.
- Before passing a report on, the node checks that `org` and `days` are the ones it asked for; that each `from` and `until` (the answer's and each team's) are `days` calendar days apart, counting both (`until` − `from` + 1 = `days`); that no team is named twice (in `teams` and in `unavailable`); that each team's days are in order and inside its own period, at most `days` of them; and that every cost is a number that is not negative. For a federation answer it also checks that the one team named is the team it asked for.
- The shared samples are `testdata/evaluation.sample.json` and `testdata/effort.sample.json`.

## Activity

What moved in a team lately, for a page that shows the work going forward: board events, pull requests, and which participant messaged which. Board events and message edges only; a message is told by its sender, its recipient, and the short summary its sender gave, and its body never leaves the team's runtime.

```
Activity      { org: Id, team: Id, generatedAt: Timestamp, events: ActivityEvent[], truncated: boolean }
ActivityEvent { id, at: Timestamp, kind, ...the fields of its kind }
itemAdded     { ref, title, assignee?, epic? }
statusChanged { ref, title, from: WorkStatus, to: WorkStatus, epic? }
prOpened      { ref, title, by?, issue?, epic? }
prMerged      { ref, title, by?, issue?, epic?, leadSeconds?: integer }
message       { from, to, summary }
```

- The events after `since`, at most `limit`, oldest first: every `at` is strictly after `since`, `at` never decreases, and events with the same `at` keep the runtime's order (by `id`). `truncated` is true when `limit` cut the list: there are events after the last one, and a read with `since` set to its `at` returns them (an event can share that time with the last one returned, so a client drops what it has by `id`).
- `id` is 1 to 64 characters of `a-z`, `0-9`, and `-` (the runtime uses a prefix and hex digits), unique in an answer and stable across answers. `ref`, `epic`, and `issue` are item references; `by`, `from`, and `to` are names of the team's runtime (`to` need not be a participant of the team); titles and names are at most 200 characters, a `summary` at most 120 (empty when the sender gave none, never taken from the message). `epic` is the parent epic of the item. `leadSeconds` is the time from a pull request's opening to its merge, when both are known. `assignee` is given only when the runtime knows it at the time.
- The times of `itemAdded`, `prOpened`, and `prMerged` are the times they happened; a `statusChanged` is the time the runtime first saw the change, up to one refresh interval late. The runtime keeps its log for a limited time (24 hours for the scrum harness); an older `since` answers what the log has.
- A reader accepts an event of a kind it does not know as left out, so the feed keeps working when a runtime learns a new kind.
- Before passing an answer on, the node checks that `org` and `team` are the ones it asked for, that there are at most `limit` events, their order and times as above, their ids, and the text limits. An answer that fails is 503.
- An answerer reads none of it (403). The route for a team on this node is in "Client API"; for a team on another node it is not answered yet (404).
- The shared sample is `testdata/activity.sample.json`.

## Client API

A client (web, iOS, desktop) talks to its home node under `/api`. The node answers as its own account: only orgs that account belongs to are visible, and anything else is 404.
The types are in `agency-proto` (`client.rs`) and generated into `bindings/`.

| Method | Path | Answer |
|---|---|---|
| GET | `/api/session` | `Session`: `writeToken`, the value writes send in `x-agency-token` (it changes each time the node starts; a client that gets 403 "missing or wrong write token" asks again once), and `now`, the node's time, which a client uses for an envelope's `at`. For clients that do not load the page; admitted like any read, so it exposes nothing the page does not |
| GET | `/api/me` | `Me`: the account (id, label, key fingerprint), its orgs with their teams (`location` local or remote, and the viewer's runtime name), and its funding |
| GET | `/api/orgs/<org>` | The org definition's body (`OrgDefinition`) |
| GET | `/api/orgs/<org>/teams/<team>/view` | `TeamSnapshot`: `view`, `viewer`, `mine` (the ids of the requests that are the viewer's to decide; see "Requests"), `waitingOnViewer` (the people who wait on the viewer; see "People"), and `source` (`local`, or `replica` with `fetchedAt`, `triedAt`, `error`). 503 when there is no replica yet |
| GET | `/api/orgs/<org>/teams/<team>/usage?days=<n>` | `TokenUsage` over the last `days` days (default 14, from 1 to 90; anything else is 400). Read from the runtime for a team on this node and live from the team's node otherwise (503 when it cannot be reached). 404 when the team's runtime does not report usage |
| GET | `/api/orgs/<org>/evaluation?days=<n>` and `/effort?days=<n>` | `Evaluation` and `Effort` for every team of the org (see "Evaluation"; `days` as for `usage`, default 14). A team on this node is worked out here and a team elsewhere is read from its node; one that cannot answer is in `unavailable`. 403 for an answerer of the org |
| GET | `/api/orgs/<org>/teams/<team>/activity?since=<time>&limit=<n>` | `Activity` for a team on this node (see "Activity"): the events after `since` (an RFC 3339 UTC time, default an hour ago), at most `limit` (1 to 500, default 200); anything else is 400. 404 when the team's runtime does not report activity or the team runs on another node, 403 for an answerer, 503 for an answer that is not valid |
| POST | `/api/orgs/<org>/teams/<team>/envelopes` | Body `SignedDecisionInput`: `envelope`, a `Signed<Envelope>` built from the request's `answerWith` and signed by one of the account's devices, plus the request's `title` and, with a choice, the chosen option's `choiceLabel`. Both are optional, trimmed, at most 200 characters, kept in the decision log for people to read, and never signed or sent to another node. The node checks the envelope's account (only its own account: 403), org, team, shape, freshness, and device signature (401), then applies it to a team on this node or sends it to the team's node. Answer `DecisionResult`: `applied`, `delivered`, or `queued` (with `message`). A runtime's 400 and 404 pass through, and so does the 403 for a request on this node that is not the account's to decide; a refusal by the team's node is 409 |
| GET | `/api/devices` | `DeviceList`: the account's devices in each of its orgs, the devices waiting to be added (`label`, `publicKey`, `deviceId`, `pairingCode`, `requestedAt`), and, for the node's owner only, `node` (below) |
| POST | `/api/devices/requests` | Body `DeviceRequestInput` (`label` of 1 to 80 characters, `publicKey`, which can't be the account key). Answer the `DeviceRequest` with its pairing code |
| POST | `/api/devices/changes` | Body `Signed<DeviceChange>` for the account, signed by one of its devices. Checked here, then written by the root's node. Answer `DeviceChangeResult`: `applied` (this node holds the root key), `delivered`, or `queued` (with `message`). A refusal by the root's node is 409 |
| GET | `/api/push/tokens` | `PushTokenList`: the owner's registered APNs push tokens, each `token` shortened to its last 8 characters, with `environment`, `label`, `registeredAt`, and `events` (the kinds it gets; a node that predates kinds has no `events`, which is how an app tells) |
| POST | `/api/push/tokens` | Body `PushTokenInput`: `token` (hex, 32 to 200 characters, stored lowercased), `environment` (`production` or `sandbox`, as the app was signed; the node never guesses), `label` (1 to 80 characters), and optionally `events`: the kinds of notification the token wants, out of `request`, `device`, and `decision` (absent means `request` only; an empty list means none; a kind named twice counts once; any other name is 400). Registering a known token updates it, `events` included. Answer the `PushToken` |
| DELETE | `/api/push/tokens/<token>` | 204, also for an unknown token. A DELETE needs the write token but has no body |
| GET | `/api/decisions?limit=<n>` | `DecisionLog`: the account's recent decisions, newest first (`limit` defaults to 50, at most 200). Each entry has `org`, `team`, `kind`, `target`, `title`, `text`, `choice`, `choiceLabel`, `decidedAt`, and `state`: `applied` (to a team on this node), `queued` (in the outbox, with `attempts`, `nextAt`, `lastError`), `refused` (by the team's node, with `message`), or `delivered`. The node keeps the last 500 decisions; decisions in orgs the account no longer belongs to are hidden |

`DeviceList.node` is `{ url, dataDir, command }`: the node's origin, the absolute data directory it runs with, and the command that approves a device at the node's terminal, so a client shows the owner the exact words and not a guess. `command` is the absolute path of the running program, `--dir` and the data directory unless that directory is `~/.agency` (the `AGENCY_DIR` variable does not count as the default, because a terminal may not have the node's environment), and `device-add`; a path that needs it is quoted for a POSIX shell. It is given only to the node's owner: a direct local request, or one through Access signed in as `ownerEmail`. Other signed-in emails read the list without it, because the program's path can hold a user name. It is left out, never `null`, when the owner is not asking or the paths cannot be written out as text; a client then shows its own wording. `agency device-add` prints the node's URL and the data directory it read before it asks anything.

Errors are `{ "error": <text> }`. JSON answers carry `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`.

Funding: each configured source (`id`, `label`, `kind` `subscription` or `apiKey`, optional `usageFile`, optional `collector`) is reported with the usage windows its file holds (`fiveHour` and `sevenDay`, each with `lengthHours`, `utilization` as a rounded percent, and `resetsAt`) and `readAt`, the time the file was last written. A missing or malformed file reports no windows. A source with a `collector` (`{ "type": "claude", "configDir"? }` or `{ "type": "codex", "codexHome"?, "command"? }`, which name where a login lives and hold no secret) has the node write the file itself, at most every 5 minutes; see "AI subscriptions and their usage" in [install.md](install.md). The node reads a collector's token only to ask its own subscription for its usage, and never stores, logs, or sends it anywhere else.

### Push notifications

When `push` is configured (`keyPath` of an APNs `.p8` key, `keyId`, `teamId`, `bundleId`), the node notifies the owner's registered push tokens through APNs. Without it, nothing is sent.

- The node checks every 30 seconds for three kinds of news, each one told once: a **request** waits for the owner, a **device** asks to be added, or a **decision** of the owner's is refused. A token gets the kinds it registered with (`events`), and a kind no token wants is recorded and sent to nobody.
  - `request`: it waits for the owner when it is the owner's to decide, by the rule in "Requests" (the same set as `TeamSnapshot.mine`, with the same limit for teams on other nodes). Its key is `org/team/request id`.
  - `device`: a device request (see "Devices") that was not waiting at the previous check, by the device's id (`d-` and its pairing code), so the same key asking again is the same device. Its key is `device/<device id>`.
  - `decision`: a decision in the decision log whose state is now `refused` and was not at the previous check, by the decision's id. A decision that was queued and is refused later is told when it is first seen refused. Its key is `decision/<id>`.
- The node remembers the last 1000 keys of each kind. The first check after the node starts with an empty data directory only records what is there. A node that held only request keys records the device requests and refusals it finds the first time, and does not send them.
- A request's notification is `{ "aps": { "alert": { "title": <team label>, "body": <request title> }, "sound": "default", "thread-id": "<org>/<team>", "badge": <count> }, "agency": { "org", "team", "request" } }` with `apns-collapse-id` set to the request id. `badge` is the number of requests waiting for the owner across all their orgs and teams when the notification is sent, this one included: the sum of `mine` over the teams whose view the node could read. The other kinds carry the same `badge`, which a device or a decision does not change.
- A device's alert is worded by the app: `{ "loc-key": "PUSH_DEVICE_WAITING", "loc-args": [<device label>] }`, with `"thread-id": "devices"`, `apns-collapse-id` the device id, and `agency: { "type": "device" }`. Only the label travels, never a key or a code. A refused decision's is `{ "loc-key": "PUSH_DECISION_REFUSED", "loc-args": [<the request's title>] }`, with `"thread-id": "<org>/<team>"`, `apns-collapse-id` the decision id, and `agency: { "type": "decision", "org", "team", "id" }`. A request has no `type`.
- When a title or label is missing (a decision logged without a title) or hidden, the alert names a string that takes no argument: `PUSH_DEVICE_WAITING_NO_LABEL`, `PUSH_DECISION_REFUSED_NO_TITLE`, and, for a request, `PUSH_REQUEST_WAITING_HIDDEN` (with no title, label, or `loc-args`). `"hideTitles": true` in `push` hides every title and label of every kind this way, for a node whose data must stay where it is. The app ships all these strings.
- Nothing else about a request, a device, or a decision leaves the node.
- A token APNs answers 410 or `BadDeviceToken` for is removed. Any other failure is tried again at the next check. The provider token (ES256 JWT, `kid` the key id, `iss` the team id) is reused for 50 minutes.

### Who is let in

The node serves the federation routes, the Client API, and the web client on one port. The federation routes are checked only by signatures. The Client API and the web client admit a request only when:

1. it is direct and local: the TCP peer is a loopback address, `Host` is `127.0.0.1:<port>` or `localhost:<port>`, and there is no `cf-ray` or `cf-connecting-ip` header. Any other `Host` gets 421, and a peer beyond loopback gets 403. The peer check is needed because the node may listen beyond loopback for federation, and the sender chooses `Host`; or
2. it comes through Cloudflare (those headers) or names the configured public host name, and carries a valid Cloudflare Access JWT in `Cf-Access-Jwt-Assertion`: RS256 only, signed by a key from `https://<team domain>/cdn-cgi/access/certs` (fetched again for an unknown key id, at most once a minute), the configured audience, issuer `https://<team domain>`, and `exp`/`nbf` within 60 seconds. Without an `access` setting, or with a malformed one, these requests get 403.

Writes (any method but GET and HEAD) also need:

- the page's token in `x-agency-token`. The node picks a random token at each start and writes it into the served `index.html` in place of the placeholder `content="__AGENCY_TOKEN__"` of `<meta name="agency-token">`. A page on another site cannot send this header without a preflight, which the node never allows;
- `Content-Type: application/json` (except for DELETE, which has no body) and a body of at most 64 KiB;
- through Access, the signed-in email must equal the configured `ownerEmail`, because writes act for the node's account: they relay its decisions and its device changes, and ask to add its devices.

The node serves `web/dist` (built into the binary) for every other path, falling back to `index.html` for the web client's own routes. Pages carry `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and `Cache-Control: no-store`.
A node with an `access` setting adds `Strict-Transport-Security: max-age=31536000` (no `includeSubDomains`) to every answer, so browsers reach its public host name over HTTPS only.
If Cloudflare Access protects the node's public host name, its policy must bypass `/fed/*`, because other nodes authenticate with signatures, not with Access, and `/.well-known/apple-app-site-association`, because Apple's servers fetch it without signing in.

### App sign-in

A web view in the iOS app cannot use passkeys for the Access team domain, so the app signs in to Access in a system browser session and takes the session over. This needs the optional setting `access.appId`, the app identifier `<team id>.<bundle id>` (ten uppercase letters or digits, a dot, and a bundle id of letters, digits, dots, and hyphens, for example `ABCDE12345.com.example.agency`). Without it, or with a malformed one, `GET /app/handoff` and the association file answer 404; the rest of the `access` setting still applies.

- `GET /app/handoff?state=<state>` is admitted like the Client API. It answers `303 See Other` with `Location: https://<publicHost>/app/callback#token=<jwt>&state=<state>`, `Cache-Control: no-store`, and `Referrer-Policy: no-referrer`, where `<jwt>` is the `Cf-Access-Jwt-Assertion` the node has just verified. The query must be exactly `state=` and 43 base64url characters (`A–Z a–z 0–9 - _`, 256 bits from the app); anything else gets 400. A direct local request gets 403 (it carries no Access sign-in), and so does a signed-in email other than `ownerEmail`. The target is built only from `publicHost`; nothing else from the request goes into it. The node never logs the request URL or the answer's `Location`.
- `GET /app/callback` is a universal link the app receives before it loads. If a browser loads it anyway, it gets a page with no script that reads nothing from the URL, admitted like any other page.
- `GET /.well-known/apple-app-site-association` is served to anyone, without an Access sign-in, as `application/json` without a redirect: `{ "applinks": { "details": [{ "appIDs": [<appId>], "components": [{ "/": "/app/callback" }] }] }, "webcredentials": { "apps": [<appId>] } }`. Nothing in it comes from the request.
- A native client keeps the handed-over token and sends it in `cf-access-token`. Once Access no longer accepts it, Access answers with a redirect to the team domain instead of the node's answer. The client never follows that redirect; it treats it as signed out and runs the handoff again.

## Test vectors

`testdata/vectors.json` holds keys from fixed seeds and the canonical JSON and signature of fixed bodies. The account key (seed `07…`) signs an org definition and a first-device change. The device key (seed `09…`, with its pairing code) signs a decision envelope and a comment (text without a choice). `canonical` lists values with the canonical text each must become: non-ASCII and astral characters, quotes, backslashes, slashes, control characters, U+007F, U+2028, precomposed next to decomposed characters (never normalized), keys whose UTF-8 order differs from their UTF-16 order, nulls, and integers. Ed25519 signatures are deterministic, so a client that imports the same seed (for example through WebCrypto) must produce the same signatures. `p256Device` is a P-256 device key (private scalar `0b…`) with its text, fingerprint, pairing code, and device id, and its signature over the comment's signed text. `deviceChangeP256` holds two device changes signed by that P-256 device: an add with a nested `device` (a second P-256 key) and a remove with a bare device id, each with its canonical JSON, its signed text (`agency/v1/device\n` and the canonical JSON), the signer's device id, and the signature, and the signer's key as `signerKey`. ECDSA signatures from other signers differ byte for byte; a client checks that these verify and that its own signature over the same text verifies with the key.
An implementation in another language must produce the same values.

`testdata/client/` holds the Client API's answers as two test nodes give them: alice's, which hosts `acme/app`, and bob's, a member that reads it as a replica. The nodes use fixed keys, counted ids, a fixed clock, and fixed ports, so the files repeat byte for byte. `cargo test` fails when a file differs from what the node answers; `AGENCY_WRITE_FIXTURES=1 cargo test -p agency-node --test fixtures` writes them again. Clients decode these files in their tests, so a change to an answer shows up as a change to a file. Besides the plain answers, it holds `me-funding.json` (`/api/me` with two funding sources), `usage.json` (a team's token usage), `device-change-result.json` and `device-change-result-queued.json` (a device change the root's node applied, and one queued while it is down), and `device-list-p256.json` (devices with an Ed25519 and a P-256 key, and a pending request from a P-256 key).
