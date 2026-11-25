# Agency の台どうしの約束 (v1)

Agency は、AI エージェントと人が一緒に働く組織を、複数の機械にまたがって扱う。
中央のサーバーは置かない。
中央にあるのは権限 (組織の決める人の鍵) だけで、データは機械ごとに分かれる。

## 単位と書き手

| 種類 | 単位 | 書き手 | 置き場所 |
|---|---|---|---|
| 実行 (稼働、要求、決裁、ボード) | チーム (ハーネス 1 つ) | そのハーネスが動いている台 | チームの台 |
| 統治 (部署、人、チームの置き場所、取り消した鍵) | 組織 | 組織の根の鍵 (決める人) | 組織のチームのある台と、所属する人の台に写す |
| 人 (鍵、所属) | アカウント | その人 | その人の家の台 |

どのデータも書き手は 1 つ。
ほかの台は写しを持つだけで、写しを書き換えない。
書き換えたいときは、書き手へ署名した封筒を届ける。

## 識別子

- アカウント、組織、チーム、部署、人の id: `[a-z0-9][a-z0-9-]{0,39}`
- 封筒の id: 16 バイトの乱数の 16 進 (32 文字)
- 時刻: RFC 3339 の UTC (`2026-11-24T12:00:00Z`)

## 鍵と署名

- 鍵は Ed25519。公開鍵は 32 バイトを base64 (標準、`=` つき) で書く。
- 鍵の指紋: 公開鍵の 32 バイトの SHA-256 の先頭 16 バイトを 16 進で書く (32 文字)。招待のリンクに入れ、組織の根の鍵が本物かを確かめる。
- 署名する文字列は「種類の行 + 決まった順の JSON」: `agency/v1/<種類>\n` に、本体の決まった順の JSON を続ける。種類は `envelope`、`org`、`join`、`read`。種類の行があるので、ある種類の署名を別の種類に流用できない。
- 決まった順の JSON: オブジェクトのキーをバイト順に並べ、空白を入れない。文字列の書き方は JSON の標準。数は整数だけを使う。値が null の項目は書かない。
- 署名したもの: `{ "body": <本体>, "signer": <アカウント id>, "signature": <base64> }`

## 組織の定義 (種類 `org`)

```json
{
  "org": "acme", "label": "Acme", "version": 3, "prev": "<前の版の指紋>",
  "root": { "account": "alice", "publicKey": "..." },
  "departments": [{ "id": "dev", "label": "開発", "parent": null, "head": "owner" }],
  "people": [{ "id": "owner", "label": "Alice", "account": "alice", "publicKey": "...", "department": "dev", "role": "member" }],
  "teams": [{ "id": "app", "label": "app", "department": "dev", "node": "https://alice.example" }],
  "revoked": ["<取り消した公開鍵>"],
  "at": "2026-11-24T12:00:00Z"
}
```

- 根の鍵 (`root`) だけが署名できる。`signer` は `root.account`。
- 版は 1 から始まり、1 ずつ増える。`prev` は前の版の署名したものの決まった順の JSON の SHA-256 の 16 進 (64 文字)。1 版は `prev` を書かない。
- 台は、今の版の次の版 (版が 1 大きく、`prev` が今の版の指紋で、今の根の鍵で署名してある) だけを受け取る。根を替えるときも、前の根の鍵で署名する。
- 所属: `people` にあり、その `publicKey` が `revoked` に無いアカウント。
- `role`: `member` (読む、答える) または `answerer` (答えるだけの人。v1 では読む範囲は member と同じ)。

## 読み出し (種類 `read`)

ほかの台のチームの様子を読むとき、見出しを付ける。

- `x-agency-account`: アカウント id
- `x-agency-date`: 時刻
- `x-agency-signature`: `agency/v1/read\n` + `<方法>\n<パス>\n<時刻>\n<アカウント>` の署名

受け取る台は、時刻が前後 5 分の内で、署名がそのアカウントの (組織の定義にある) 公開鍵で合い、そのアカウントがチームの組織に所属しているときだけ返す。
所属していなければ 404 (チームがあることも見せない)。

## 決裁の封筒 (種類 `envelope`)

```json
{ "id": "<32 文字>", "kind": "answer", "org": "acme", "team": "app", "target": "r-20260101-000000-abcd",
  "payload": { "text": "", "choice": "ア" }, "account": "alice", "at": "2026-11-24T12:00:00Z" }
```

- `kind`: `answer` (要求に答える、`target` は要求の id) または `judge` (ボードの判断待ちに答える、`target` は `repo#番号`)。
- 受け取る台 (チームの台) は、署名、所属、期限 (作ってから 7 日)、届け先のチームと封筒の組織とチームが合うこと、同じ id の 2 度目でないことを確かめてから当てる。2 度目は当てずに受け取ったと答える。
- 送る台は、届かなければ送り箱に残し、間を空けて送り直す (15 秒、30 秒、1 分、2 分、5 分、以後 5 分)。相手が 4xx で断ったら送り直さない (408 と 429 は送り直す)。
- Who signs: today the home node signs envelopes for its owner with the account key, including decisions made through the Client API. agency#19 moves signing to device keys held by clients before other people join an org or the iOS client ships.

## 招待と参加 (種類 `join`)

1. 根の鍵の台で招待を作る。招待のリンク: `agency:join?node=<台の頭の URL>&org=<組織>&fp=<根の鍵の指紋>&code=<合言葉>`。合言葉は 16 バイトの乱数の base64url。台には合言葉の SHA-256 だけを残す。期限は 7 日、使えるのは 1 回。
2. 招かれた人の台が `GET <台>/fed/v1/orgs/<組織>/definition?code=<合言葉>` で組織の定義を読み、根の鍵の指紋がリンクの `fp` と合うかを確かめる。合わなければ止める (偽の台)。
3. 招かれた人が受けると、参加の申し込みを署名して `POST <台>/fed/v1/orgs/<組織>/join` に送る。本体: `{ "org", "code", "account", "label", "publicKey", "at" }`。署名は本体の `publicKey` で確かめる。
4. 根の鍵の台は、合言葉、期限、使った回数を確かめ、招待で決めた位置 (部署、役) にその人を書き足した次の版に署名し、申し込んだ台に返す。新しい版は、組織のチームのある台へ送り箱で配る。

## 台の道

| 方法 | パス | 確かめ |
|---|---|---|
| GET | `/fed/v1/orgs/<組織>/definition` | 所属する人の読み出しの署名、または有効な招待の合言葉 |
| POST | `/fed/v1/orgs/<組織>/definition` | 次の版であること (根の鍵の署名) |
| POST | `/fed/v1/orgs/<組織>/join` | 参加の申し込みの署名と招待 |
| GET | `/fed/v1/teams/<組織>/<チーム>/view` | 読み出しの署名と所属 (返すのは `{ "view": TeamView, "viewer"?: <名前> }`) |
| POST | `/fed/v1/teams/<組織>/<チーム>/envelopes` | 封筒の署名、所属、期限、2 度目でないこと |

## TeamView

A TeamView is what a team runtime reports about its team: its people, the requests waiting for a decision, its work items, and its epic forecasts.
The node reads it from the team runtime (`view` in agency-runtime/1), returns it from `GET /fed/v1/teams/<org>/<team>/view` as `{ "view": TeamView, "viewer"?: <runtime name> }`, and serves it to clients.
`viewer` is the signed reader's name in the team runtime (see "Requests" below for the mapping); it is absent when the team does not address the reader.
A node that reads another node's TeamView parses it into these types before keeping a replica, and drops it if it does not parse.

The Rust types in `agency-proto` (`view.rs`, `runtime.rs`, `timestamp.rs`) are the source of truth.
The TypeScript types in `bindings/` are generated from them with ts-rs and are never edited by hand; `scripts/check-bindings.sh` regenerates them and fails on any difference.
`testdata/teamview.sample.json` is the shared sample: the Rust tests, the web client, and the harness all read it.

Conventions:

- Keys are camelCase. Numbers are non-negative integers.
- `Timestamp`: RFC 3339 in UTC, ending in `Z` (`2026-11-24T12:00:00Z`; fractional seconds are allowed). `Date`: a calendar date, `YYYY-MM-DD`. They are different types.
- An optional field is omitted when absent. It is never written as `null`.
- People and addressees are named by the team runtime's own names (for example a session name, or `owner`), not by Agency org person ids. The team runtime does not know org person ids.
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
ThreadMessage { from, text, choice?: string, at: Timestamp }
WorkItem      { ref /* repo#N */, title, status: "backlog" | "next" | "inProgress" | "release" | "done",
                assignee?: string, blocked?: { reason: BlockedReason, since: Date }, epic?: string,
                points?: number, eta?: { p50: Timestamp, p80: Timestamp, basis, isOverBudget: boolean } }
BlockedReason "needsDecision" | "external" | "waitingOnPrevious" | "timeOrEquipment"
EpicForecast  { ref, title, total, done, doing, remainingPoints, blocked: { reason: BlockedReason, count }[],
                forecast?: { days, at: Timestamp }, basis }
```

People:

- `state` says whether the person is active; `waitingOn` says who has to act before they can continue. A person can be `working` and still wait on someone, for example on a permission prompt: then `waitingOn` names the person who answers the prompt and `status` says it is a permission prompt.
- `waitingOn.person` uses the same names as `Request.to`.

Requests:

- `question`: the team asks something. `proposal`: the team asks for approval of a document (`document`) with options. `pendingDecision`: a board item is blocked until someone decides.
- `answerWith` is exactly the `kind` and `target` of the decision envelope that answers the request, so clients never derive them. A `pendingDecision` is answered with `judge` and its `repo#N`; the others with `answer` and the request id.
- `to` absent means the team's owner. The node maps runtime names to org people through its hosted-team configuration: `owner` is the org person id of the team's owner, and the optional `people` map names other runtime names (`{ "<runtime name>": "<org person id>" }`). A `to` that is not in `people` goes to the owner.
- In the other direction, an org person's runtime name is their key in `people`; otherwise `owner` if they are the team's owner; otherwise they have none in that team.
- `context`, `detail`, `status`, and messages are plain text. `document` is Markdown; clients render a safe subset and never render raw HTML from it.
- `thread` holds earlier messages, oldest first, when a request was answered and then reopened.

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
| `decide` | `"decision": { "kind": "answer" \| "judge", "target", "by": <org person id>, "payload": { "text", "choice"? } }` | `{}` |

- The node calls `describe` first and stops unless `runtimes` contains `agency-runtime/1`. Unknown names in `ops` are ignored.
- The node calls `decide` only after it has verified the decision envelope (signature, membership, expiry, team, replay). `by` is the org person who decided.
- Error statuses: 400 (malformed input), 404 (no such request or work item), 503 (the runtime cannot act now). The node treats 400 and 404 as refusals and does not retry them; it retries every other failure, including a timeout or output that is not a response.
- `usage` is a reserved op name. Subscription usage belongs to the account, not to a team runtime, and is not part of agency-runtime/1.

## Client API

A client (web, iOS, desktop) talks to its home node under `/api`. The node answers as its own account: only orgs that account belongs to are visible, and anything else is 404.
The types are in `agency-proto` (`client.rs`) and generated into `bindings/`.

| Method | Path | Answer |
|---|---|---|
| GET | `/api/me` | `Me`: the account (id, label, key fingerprint), its orgs with their teams (`location` local or remote, and the viewer's runtime name), and its funding |
| GET | `/api/orgs/<org>` | The org definition's body (`OrgDefinition`) |
| GET | `/api/orgs/<org>/teams/<team>/view` | `TeamSnapshot`: `view`, `viewer`, and `source` (`local`, or `replica` with `fetchedAt`, `triedAt`, `error`). 503 when there is no replica yet |
| POST | `/api/orgs/<org>/teams/<team>/decisions` | Body `DecisionInput` (`kind` and `target` from the request's `answerWith`, `text`, `choice`, and optionally the request's `title`, at most 200 characters, kept in the decision log). Answer `DecisionResult`: `applied`, `delivered`, or `queued` (with `message`). A runtime's 400 and 404 pass through; a refusal by the team's node is 409 |
| GET | `/api/decisions?limit=<n>` | `DecisionLog`: the account's recent decisions, newest first (`limit` defaults to 50, at most 200). Each entry has `org`, `team`, `kind`, `target`, `title`, `text`, `choice`, `decidedAt`, and `state`: `applied` (to a team on this node), `queued` (in the outbox, with `attempts`, `nextAt`, `lastError`), `refused` (by the team's node, with `message`), or `delivered`. The node keeps the last 500 decisions; decisions in orgs the account no longer belongs to are hidden |

Errors are `{ "error": <text> }`. JSON answers carry `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`.

Funding: each configured source (`id`, `label`, `kind` `subscription` or `apiKey`, optional `usageFile`) is reported with the usage windows its file holds (`fiveHour` and `sevenDay`, each with `lengthHours`, `utilization` as a rounded percent, and `resetsAt`). A missing or malformed file reports no windows. The node never reads or stores a token.

### Who is let in

The node serves the federation routes, the Client API, and the web client on one port. The federation routes are checked only by signatures. The Client API and the web client admit a request only when:

1. it is direct and local: the TCP peer is a loopback address, `Host` is `127.0.0.1:<port>` or `localhost:<port>`, and there is no `cf-ray` or `cf-connecting-ip` header. Any other `Host` gets 421, and a peer beyond loopback gets 403. The peer check is needed because the node may listen beyond loopback for federation, and the sender chooses `Host`; or
2. it comes through Cloudflare (those headers) or names the configured public host, and carries a valid Cloudflare Access JWT in `Cf-Access-Jwt-Assertion`: RS256 only, signed by a key from `https://<team domain>/cdn-cgi/access/certs` (fetched again for an unknown key id, at most once a minute), the configured audience, issuer `https://<team domain>`, and `exp`/`nbf` within 60 seconds. Without an `access` setting, or with a malformed one, these requests get 403.

Writes (any method but GET and HEAD) also need:

- the page's token in `x-agency-token`. The node picks a random token at each start and writes it into the served `index.html` in place of the placeholder `content="__AGENCY_TOKEN__"` of `<meta name="agency-token">`. A page on another site cannot send this header without a preflight, which the node never allows;
- `Content-Type: application/json` and a body of at most 64 KiB;
- through Access, the signed-in email must equal the configured `ownerEmail`, because the node signs decisions with its owner's account key.

The node serves `web/dist` (built into the binary) for every other path, falling back to `index.html` for the web client's own routes. Pages carry `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and `Cache-Control: no-store`.
If Cloudflare Access protects the node's public host name, its policy must bypass `/fed/*`, because other nodes authenticate with signatures, not with Access.

## 試験の値

`testdata/vectors.json` に、決まった種の鍵、本体、決まった順の JSON、署名を置く。
別の言語で作り直すときも、同じ値になることを確かめる。
