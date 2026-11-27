# Glossary

One word for one thing, in the documents, the code's comments, and the screens.
Each term has its Japanese name: the one the UI uses, or, marked *(proposed)*, the one it should use.
A term with no Japanese name is not shown to people.

Wire and type names (`people`, `Person`, `PersonStatus`, `answerWith`, ...) are part of the protocol and stay as they are, even where the word differs from the term here. The entries say so where it happens.

## People and accounts

- **Human** (人). A real person. The system does not represent humans. One human has one or more accounts, for example a personal one and a work one, one for each boundary the data must not cross. Nothing in the system links two accounts of one human; only a client that holds both does. (Today a client talks to one home node, so none does.)
- **Account** (アカウント). One human's identity in the system: an id, a label, and an account key (Ed25519). It belongs to exactly one human and is never shared, because a device signature attributes a decision to the human behind the account. The account key signs reads, join requests, and the account's first device in an org. It never signs a decision.
- **Home node** (ホームノード). The node an account lives on. It holds the account key, keeps replicas of what the account sees, and serves the account's clients under `/api`. A node's configuration holds exactly one account (`account` in `config.json`), so a node is the home node of one account, and an account's home node is the one node that holds its key. The code has no register of this, so "exactly one per account" is a rule for the owner, not a check. One browser or the app can list several home nodes. Each is its own origin in a browser, with its own sign-in and its own device key, and nothing is copied between them.
- **Member** (メンバー). An account's seat in one org: `id`, `label`, `account`, `publicKey`, `department`, `role`, and `devices`. An account has no member, or one, in each org (an org definition refuses two members with the same account), so it has as many members as orgs it belongs to. In the protocol a member is `people[]` of the org definition, with the type `Person`; the names stay for now. A member's `role` is `member` (reads and answers) or `answerer` (only answers).
- **Participant** (参加者). One who takes part in a team's work: a human or an agent, as the team's runtime reports it (`TeamView.people[]`, the type `PersonStatus`, `kind` `human` or `agent`). A participant is named by the runtime's own name, not by a member id. The two are matched only through the hosted team's configuration. This is a third thing the protocol calls "person"; the screen "Participants and work" (参加者と仕事) lists them. A participant's `role` is `lead` or `member`: a *team role*, reported by the team's runtime. The screens show only the lead (リード) and do not name the other role, because "member" is the org seat; the protocol keeps the value `member` for now. Do not confuse it with a member's org role (`member` or `answerer`).
- **Owner**. Two uses, both in the code: the *team owner* is the member whose answer an unaddressed request waits for (`owner` of a hosted team, and the runtime name `owner`); the *node's owner* is the human whose email Cloudflare Access must show for writes (`ownerEmail`). Say which one when it is not clear.
- **Viewer** (閲覧者 *(proposed)*). The account that reads a team: its runtime name in that team is `viewer`, and `mine` and `waitingOnViewer` are computed for it.

## Machines and nodes

- **Machine** (マシン). The computer a node runs on. A machine can run several nodes (each with its own data directory).
- **Node** (ノード). One running `agency` with its data directory: its account key, its replicas, its outbox, its configuration. It serves federation routes, the Client API, and the web client on one port.
- **Node origin**. The URL other nodes reach a node at (`url` in `config.json`, `teams[].node`, an invite's `node`).
- **Public host name** (ホスト名). The name a node is reached at from outside, through Cloudflare (`access.publicHost`). It is a name, never a machine. The node's origin and its public host name can be the same host name, but they are two settings.

## Orgs and teams

- **Org** (組織). A set of departments, members, and teams under one root key. Its definition is the signed `OrgDefinition`.
- **Root key** (ルート鍵). The Ed25519 key that signs an org's definition versions. It is held by the root's node (`root` in the definition names its account and key).
- **Recovery key** (回復用の鍵). A second key of an org, named in `root.recovery`, that no machine keeps: the owner keeps it as written **recovery words** (回復用の単語). It may only move the root key or itself, and only after a **recovery** (ルート鍵の回復) waits 72 hours, in which the root key cancels it by signing any version.
- **Org definition** (組織定義) and its **versions**. The signed body listing the org's departments, members, teams, and revoked keys. Version 1 starts it; each next version is one higher and refers to the one before (`prev`). Every piece is written only by the root key.
- **Department** (部署). A unit of the org that groups members and teams, with an optional parent and an optional head (a member).
- **Team** (チーム). One harness, running on exactly one node: the *team's node* (`teams[].node`). Requests, activity, and the board belong to a team, and the team's node is their only writer.

## Devices and keys

- **Device** (端末). A key held by a client (a browser, a phone, the CLI) that signs decisions and changes to the member's devices. It is listed under a member (`devices[]`), so it belongs to one member in one org. A device key is Ed25519 or P-256 and never leaves the client; a node never holds one.
  - Per org or per account? In the data, per org: each org's definition lists the device under its member, and each addition is its own signed change for one org. A device key may be listed in several orgs, but is unique in one. Clients and `agency device-add` add a new device in every org the account belongs to, in one go, and a device request stays open until it is added everywhere. A device that is not listed in an org cannot sign there and cannot add anything there, so after joining another org, the account's first device there is added from the node's terminal with the account key, and later ones from a device that is listed in that org.
- **Pairing code** (照合コード). The first 12 hex characters of a device key's fingerprint, in groups of 4. The person compares it on both devices before adding.
- **Approve a device** (承認). To let a device that asked to be added in. The first device of an org is approved on the node's machine, with the account key (`agency device-add`); once the account has a device in an org, a later one is approved from one of those devices, which compares the pairing codes and signs the change. The screen of a device that waits says which of the two applies to it, and for this node, with its exact command.
- **Device id**. `d-` and the pairing code without dashes.
- **Client** (クライアント). A program with device keys that talks to a home node: the web client, the iPhone app, the CLI on the node's terminal. A client is not a node.
- **Push token**. An APNs token for the iPhone app to be notified at. Apple calls it a device token; the protocol does not, since a *device* here is a signing key.

## Seats and claiming

- **Seat** (席). A place in an org that has been given a name and a department but that no one has taken yet. A seat has no account, no key, and no devices, so it is **not a member**: it is addressed, but it never decides. In the protocol it is `seats[]` of the org definition, with `askAbout` (what to ask it about) and `relays` (who answers for it). Say *seat* for it and keep *member* for one who has taken a seat, because a member has an account (or is hosted, below). When someone takes a seat it leaves `seats[]` and its id stays with that member as `formerly`, so its requests and history stay attached and a request addressed to it is theirs.
- **Claim link** (席の受け取りリンク *(proposed)*). A link, for one seat, for one person, that works once and expires, which the org's root node makes. Joining with it (`agency join`; the root's node makes it with `agency invite --seat`) makes the person a member in place of the seat, in its department and with what to ask about it. A page that opens it, signing in with the address it was made for and confirming a pairing code, is not built yet. The code in the link is secret: a page reads it from the address and removes it at once, and sends it only to the node, in the request that claims the seat.
- **Hosted member** (ノードを持たないメンバー). A member who has no node of their own: their client talks to the org root's node, which holds no key of theirs. Their devices hold their keys and sign their decisions, as for every member. They read only the requests addressed to them.
- **Guest** (ゲスト). A hosted member signed in to the root's node through Access as itself. It reaches only its own requests, devices and decisions there, and never acts as the node's owner. In the code it is `Admitted::Guest` and `guests::Guest`; the email that is which member is kept on the root's node, never in the definition.
- **Answer for** (代理で答える). To record an answer on behalf of a seat or an external party who does not answer themselves. The protocol field `relays` names who may. Do not use *relay* for this: a node relays a change or an envelope to another node, and a screen that says "relayed for" will be read as that.
- **External party** (外部の相談先). Someone the org asks through a member and who never takes a seat. In the protocol it is `externals[]` of the org definition, with `askAbout` and a required `relays`.

## Requests and decisions

- **Request** (依頼). What a team asks of a member. Its kinds (`Request.kind`):
  - **Proposal** (稟議, `proposal`): approval of a document, with options. 稟議書 means the document itself, and only that.
  - **Question** (質問, `question`): the team asks something, with options or free text.
  - **Pending decision** (判断待ち, `pendingDecision`): a board item is blocked until someone decides. It is answered with `judge`.
- **Item reference**. The opaque string that names a work item, chosen by the team's harness: `WorkItem.ref` on the wire, and the same string in an epic's `ref`, a request's `issue`, and a participant's `holding`. Nothing in Agency parses it. The one that decides a pending decision (`answerWith.target`) must be 1 to 120 bytes of letters, digits, and `._#-`, so a runtime maps a tracker's id that does not fit into one that does (see "Item references" in the protocol). Not shown to people as a term.
- **Envelope**. The signed document that carries a decision to the team's node (`kind` `answer` or `judge`). Not shown to people.
- **Decision** (決定). A member's answer to a request, sent as an envelope signed by one of the member's devices. It is also what the log lists ("Recently decided" / 最近の決定).
- **Kept decision** (送信中の決定, the name of the list). A decision signed on a client that the node has not **taken** yet. The client keeps the signed envelope on the device (a reload, a closed tab or a killed app loses nothing) and sends it as often as it takes: always the same envelope with the same id, which the node keeps once. The request it answers stays off the pile and off every count while the decision is kept. The list of them is the **sending tray**: EN "Sending decisions", JA 「送信中の決定」. A kept decision is `sending` (EN "Sending…", JA 「送信中…」), `retrying` (EN "Not sent yet. Next try …", JA 「まだ送れていません。次の再送は … です。」) or `failed` (EN "Not sent: <reason>", JA 「送れませんでした: <理由>」); `expired` (the node refuses an envelope older than 7 days) is a reason, not a state. **Send again** (もう一度送る) is on every kept decision that did not get through: the same envelope, at once. **Discard** (破棄) is only on a failed one: the request returns to the pile with what was typed and chosen. These states belong to the client and are not the decision log's. To keep a decision on the device is JA 「この端末に残す」 (for example 「この端末に決定を残せなかったため、送っていません。」 when it cannot be written); never 「保存」, which is the node's word for `queued` (保存済み・未配達).
- **Taken** (受け取った). The node answered a kept decision with any 2xx (`applied`, `delivered` or `queued`). The decision then leaves the tray, and what becomes of it is the decision log's state. Taken is not a state a screen shows.
- **Queued** (`queued`). The decision log's state for a decision that the **node** holds and has not yet passed on to the team's node or runtime, which it tries again by itself. On screens it is EN "Saved, not delivered", JA 「保存済み・未配達」, with the detail "Not delivered yet." / 「まだ届いていません。」 until the node has tried. While the log says `queued`, the request also stays off the pile and off the counts. It is never a state of a kept decision on a client, whose words are about *sending* (to the node), while the node's are about *delivering* (to the team).
- **Approval** (決裁). The decision on a proposal, and only on a proposal: the verb is 決裁する, the stamp 決裁. A question is answered (答える, 回答), and a pending decision is judged (判断を送る).
- **Comment** (コメント). Text sent with no option picked, on a request that has options. The turn goes back to the team and nothing is decided. In the protocol it is an `answer` envelope with text and no `choice`; the protocol and the node do not name it, and a request without options takes its text as the answer, so the word belongs to the clients.
- **Mine** and **waiting on the viewer**. Two lists the home node computes for a viewer (`TeamSnapshot.mine`, `TeamSnapshot.waitingOnViewer`); clients show them and never work them out.

## How data moves

- **Replica** (写し). A copy of a team's view, or of an org definition, that a node keeps of what another node writes. It is never edited, and says when it was read.
- **Outbox**. A node's queue of envelopes, device changes, and org versions that could not be delivered yet; it retries with backoff. No UI name of its own; the decision log shows a decision it holds as `queued` (保存済み・未配達). It is not the client's list of kept decisions (see **Kept decision**).
- **Federation** (連合). Nodes reading and writing each other's data through the `/fed/v1` routes, authenticated by signatures.
- **Agent** (エージェント). A participant that is not a human: an AI that works in a team.
- **Harness**. The program that runs a team's agents and keeps its work (for example the scrum harness). One team is one harness.
- **Runtime**. The command a node runs for a hosted team to read its view and to pass a decision to it, speaking `agency-runtime/1`. The node never reads a harness's files; it talks to the runtime.

## Evaluation

The numbers are worked out by the node and served at `/api/orgs/<org>/evaluation`; a screen only draws them, and an agent reads the same route to cite them in a proposal.

- **Evaluation** (評価). How the development organization performs: first how fast new features ship (merged pull requests by type, lead times, red main, reverts, cost per merged pull request), then where effort goes.
- **Lead time** (リードタイム). The time from a start to the merge of a pull request, in two kinds: the *pull request's lead time* (the pull request opened to merged) and the *issue's lead time* (the issue started to merged; started is when the board moved the issue to In Progress, else the pull request's first commit). The screen shows the median (中央値) and the p90: the smallest lead time that at least 9 in 10 of the period's pull requests do not exceed (nearest rank).
- **Main red** (main が赤). The CI of the `main` branch is failing, from the first failed push run to the next one that passes. The screen shows the time it was red in the period and the longest spell.
- **Revert** (リバート). A merged pull request that reverts another: its title starts with `Revert`, or its body has a line that starts with "This reverts commit <sha>" or "Reverts <owner/name>#<number>". The node counts it on its own, not in a type, so the merged counts plus the reverts are every merged pull request.
- **Cost per merged pull request**. The period's token cost, API-priced as in a token usage report, divided by the pull requests merged in it (the reverts included). It is not attributed to each pull request: token use is known by day and team only.
- **Work type** (作業種別). What a piece of work is for: new feature (新機能), improvement (改善), maintenance (保守), tech debt (技術的負債), or incident (障害対応). It comes from the board's "Work type" field and, when an item has none, from its pull request's Conventional Commits type, by the rule the node documents (the node computes the split, not the screen). What neither classifies is *unclassified* (未分類); it is shown as such and never guessed. The wire names are `newFeature`, `improvement`, `maintenance`, `debt`, `incident`. A work item in a team's view carries the board's value as `workType`, and has none when the board has none (the view never carries the pull-request guess).
- **Plan** (計画). What the org intends for a team (not a subscription's plan, the product tier of a funding source, which the costs screen calls a plan too), in `teams[].plan` of the org definition, signed by the root key: the *allocation* (割合), the share of the team's effort for each work type, whole numbers that sum to 100, and the *weekly budget* (週の予算), whole US dollars at API prices. A team may have none; the screen then compares nothing.
- **Weekly budget** (週の予算). The most a team may spend in a week, at API prices: `weeklyBudgetUsd` of its plan. Where a team stands on it is the node's answer (`GET /api/orgs/<org>/teams/<team>/budget`, and `agency budget status`, the same function): the week is the current one from Monday in the node's time zone, the state is `ok`, `warning` (from 80% of the cap) or `over` (at or past it), a team is `over` when its org is, and a usage that could not be read marks the amount `partial`, which a screen says is at least that much. A screen shows that answer and computes no week and no threshold of its own. Not the *Budget* of an in-node read (a response time), and a screen that shows both says which.
- **Effort** (労力). What a team spent, by work type: the token cost, API-priced as in a token usage report, served at `/api/orgs/<org>/effort`.

## Outcomes

What the org's work achieves, as numbers about the business or the product, not about delivery (that is the evaluation). What the org measures is stated in its signed definition, with no number in it; the numbers are business data and stay on the owner's node.

- **Outcome** (成果). What an org's work achieves, told by numbers about its business (revenue, margin, churn) or its product (activation, retention, lead time of a feature, a latency budget). Not the delivery numbers of the evaluation.
- **KPI** (KPI). A measure the org states in `kpis[]` of its definition: an `id`, a `label`, its `kind` (`business` or `product`), its `unit`, and whether a `higher` or a `lower` value is better. It holds no number and no target figure. A `money` KPI names its currency by an ISO 4217 code (`JPY`), and there is no conversion: the org measures in the currency it reports in. The other units are `percent` (a number of percent: 41.5 means 41.5%), `count`, `ratio` (a plain number: 1.5 means 1.5 times), and `seconds` (a duration in seconds).
- **Observation** (実績値). One number for a KPI on one day, with where it came from (entered by hand, or from a connector), who entered it, and when. It is kept on the owner's node only, never in the definition, and only the owner reads it.
- **Reading** (読み取り). How a KPI's numbers stand around the day the epic an aim names shipped: the last number on or before that day, the first at least 14 days after it, and their change. It is a *correlation*, never a cause, and it refuses (with a state that says why) when there is too little to read. Read at `/api/orgs/<org>/outcomes/readings`, owner only.
- **Aim** (狙い). What a team's plan says an epic is meant to move: `plan.aims[]` pairs an epic (by its item reference) with a KPI of the org.
- **Reading state** (読み取りの状態). Why a reading has numbers, or why it has none. The values of `state` on the wire are `ready`, `notShipped`, `shipDateUnknown`, `noBaseline`, `tooEarly`, `noFollowUp`, and `tooFew` (they are identifiers, not screen text); only `ready` has numbers. Each of the others is said on screen by a sentence of the client that tells what is missing, in `outcomes.readings.states` of the web's catalogs and in the Apple apps' wording of the same, and `ready` has no state sentence: it shows its two numbers, the change, and the correlation line. A screen draws no trend and no 0 for a state that refuses.
- **Movement** (動き). The change of a KPI's number, worded by direction (rose 上がった, fell 下がった, unchanged 変わらず) and, from the KPI's `goodWhen`, as better (良い方) or worse (悪い方); a KPI with a `goodWhen` a client does not know is not coloured, and a change that is written as 0 in the KPI's unit is unchanged. A reading's movement is never worded as caused by the work: the screens say 「完了のあとで数値が動きました。この仕事が動かしたとは言えません。」 (English: "The figure moved after the work shipped. That does not say the work moved it.").
- **Outcomes page** (成果の指標; English "Outcomes"). The owner's screen of the KPIs' numbers and the readings. Its Japanese name is not 成果 alone, because the Results group of the menu it sits in (English "Results", 成果) has that name.

## The node's own measurements

What a node measures of itself, kept on the node and read only by its owner (`/api/metrics`, "Metrics" in `docs/protocol.md`).

- **Response time** (応答時間). How long a node takes to answer a request, or how long a call it makes takes to be answered. The screens say response time, not *latency*.
- **Route group** (ルートの区分). The area of the Client API a route belongs to (`desk`, `teams`, `usage`, `evaluation`, ...), as the node says it in `group`; a call the node makes has the group of its first word (`runtime`, `peer`, `push`, `github`). The screens give each its own name.
- **In-node read** (ノード内の読み取り). A read the node answers from its own state, without asking the team's runtime, a peer, or GitHub (`isInNodeRead`). Only these have a response-time budget.
- **Budget** (応答時間の予算), in this section only: the response time an in-node read must keep, 100 ms at the 95th percentile in the reference environment (`budget.inNodeReadP95Ms`). Not the cost budget of a team's plan (`weeklyBudgetUsd`): a screen that shows both says which.
- **Objective** (目標). A response time the node's operator sets for a route group (`metrics.objectives` in `config.json`: a group and a `p95Ms`); the node says whether it was `met` or `notMet` over the days asked, or `unknown` when it cannot tell, with what it measured.
- **p50, p95, p99**. The response time that 50, 95, or 99 of every 100 requests were within, as the node gives it: **the upper bound of the bucket** the request fell in, so a screen says "at most", never an exact time; one beyond the last bucket (30 000 ms) has no bound and is shown as slower than 30 seconds.

## Do not use

| Do not say | Say | Why |
|---|---|---|
| host, for a computer; ホスト (ホスト名 is fine) | machine (マシン) | A host is a name a node is reached at (`publicHost`), and the HTTP `Host` header keeps its name. |
| 拠点 | ノード or マシン | It does not say which. |
| computer; コンピュータ, コンピューター | machine (マシン) | One word for the computer a node runs on. |
| server, for a node; サーバー, サーバ | node (ノード) | A node is a peer. There is no central service. |
| user; ユーザー, ユーザ | account (アカウント) for the identity; human (人) for the real person; member (メンバー) for the seat in an org | It does not say which of the three. |
| デバイス | 端末 | The Japanese name of a device is 端末. |
| 判断のお願い | 判断待ち | The kind `pendingDecision` has one name. A sentence may say 「…さんの判断待ち」. |
| person, for a member | member (メンバー) | "Person" stays for the human. |
| unclaimed member; 未参加のメンバー | seat (席) | A seat is not a member: it has no account, key, or device. |
| relay, for answering on someone's behalf | answer for (代理で答える) | A node relays changes to another node; a person answers for someone. |
| person, for a participant; 人 for a participant or a count of them | participant (参加者) | A participant may be an agent. |
| device, for a push token | push token | A device is a signing key. |
| queue or outbox, for a decision a client keeps; 送信待ち and "Waiting to send" for anything | sending decisions (送信中の決定) for the client's list; Saved, not delivered (保存済み・未配達) for the node's `queued` | *Sending* is the client's leg to its node and *delivered* the node's leg to the team, so each word says one thing. 送信待ち was the log's word for `queued` and sounded like the tray's; the outbox is the node's. |
| pending, for a kept decision | `sending`, `retrying`, `failed` | `pending` is already the outcome of a device change held for the root key, and `pendingDecision` (判断待ち) a kind of request. |
| sent (送った), for a decision the node has not taken | not sent yet (まだ送れていません) | A decision is sent when the node has taken it; until then the screen says it has not been. |

"Host" is also fine as the verb of a node that runs a team's runtime (a *hosted team*), and in the `Host` header.

`scripts/check-words.sh` fails the build on the words above that a machine can tell apart from a good use (the Japanese ones in `web/src/i18n/ja.ts` and `docs/*.ja.md`, and the English ones in `web/src/i18n/en.ts`). The others (人 for a participant, 稟議書 for anything but the document itself) are left to review. Three phrases are let through because they are not about a node or a machine of ours: ホスト名 (the public host name), セルフホスト (Cloudflare's self-hosted application type), and Apple のサーバー (Apple's own servers).
