# Glossary

One word for one thing, in the documents, the code's comments, and the screens.
Each term has its Japanese name: the one the UI uses, or, marked *(proposed)*, the one it should use.
A term with no Japanese name is not shown to people.

Wire and type names (`people`, `Person`, `PersonStatus`, `answerWith`, ...) are part of the protocol and stay as they are, even where the word differs from the term here. The entries say so where it happens.

## People and accounts

- **Human** (人). A real person. The system does not represent humans. One human has one or more accounts, for example a personal one and a work one, one for each boundary the data must not cross. Nothing in the system links two accounts of one human; only a client that holds both does. (Today a client talks to one home node, so none does.)
- **Account** (アカウント). One human's identity in the system: an id, a label, and an account key (Ed25519). It belongs to exactly one human and is never shared, because a device signature attributes a decision to the human behind the account. The account key signs reads, join requests, and the account's first device in an org. It never signs a decision.
- **Home node** (ホームノード). The node an account lives on. It holds the account key, keeps replicas of what the account sees, and serves the account's clients under `/api`. A node's configuration holds exactly one account (`account` in `config.json`), so a node is the home node of one account, and an account's home node is the one node that holds its key. The code has no register of this, so "exactly one per account" is a rule for the owner, not a check.
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

## Requests and decisions

- **Request** (依頼). What a team asks of a member. Its kinds (`Request.kind`):
  - **Proposal** (稟議, `proposal`): approval of a document, with options. 稟議書 means the document itself, and only that.
  - **Question** (質問, `question`): the team asks something, with options or free text.
  - **Pending decision** (判断待ち, `pendingDecision`): a board item is blocked until someone decides. It is answered with `judge`.
- **Envelope**. The signed document that carries a decision to the team's node (`kind` `answer` or `judge`). Not shown to people.
- **Decision** (決定). A member's answer to a request, sent as an envelope signed by one of the member's devices. It is also what the log lists ("Recently decided" / 最近の決定).
- **Approval** (決裁). The decision on a proposal, and only on a proposal: the verb is 決裁する, the stamp 決裁. A question is answered (答える, 回答), and a pending decision is judged (判断を送る).
- **Comment** (コメント). Text sent with no option picked, on a request that has options. The turn goes back to the team and nothing is decided. In the protocol it is an `answer` envelope with text and no `choice`; the protocol and the node do not name it, and a request without options takes its text as the answer, so the word belongs to the clients.
- **Mine** and **waiting on the viewer**. Two lists the home node computes for a viewer (`TeamSnapshot.mine`, `TeamSnapshot.waitingOnViewer`); clients show them and never work them out.

## How data moves

- **Replica** (写し). A copy of a team's view, or of an org definition, that a node keeps of what another node writes. It is never edited, and says when it was read.
- **Outbox**. A node's queue of envelopes, device changes, and org versions that could not be delivered yet; it retries with backoff. No UI name; if one is needed, 送信待ち.
- **Federation** (連合). Nodes reading and writing each other's data through the `/fed/v1` routes, authenticated by signatures.
- **Agent** (エージェント). A participant that is not a human: an AI that works in a team.
- **Harness**. The program that runs a team's agents and keeps its work (for example the scrum harness). One team is one harness.
- **Runtime**. The command a node runs for a hosted team to read its view and to pass a decision to it, speaking `agency-runtime/1`. The node never reads a harness's files; it talks to the runtime.

## Evaluation

The numbers are worked out by the node and served at `/api/orgs/<org>/evaluation`; a screen only draws them, and an agent reads the same route to cite them in a proposal.

- **Evaluation** (評価). How the development organization performs: first how fast new features ship (merged pull requests by type, lead times, red main, reverts, cost per merged pull request), then where effort goes.
- **Lead time** (リードタイム). The time from a start to the merge of a pull request, in two kinds: the *pull request's lead time* (the pull request opened to merged) and the *issue's lead time* (the issue started to merged; the node decides what counts as started). The screen shows the median (中央値) and the p90: the smallest lead time that at least 9 in 10 of the period's pull requests do not exceed (nearest rank).
- **Main red** (main が赤). The CI of the `main` branch is failing, from the first failed run to the next one that passes. The screen shows the time it was red in the period and the longest spell.
- **Revert** (リバート). A merged pull request that reverts another.
- **Cost per merged pull request**. The period's token cost, API-priced as in a token usage report, divided by the pull requests merged in it.
- **Work type** (作業種別). What a piece of work is for: new feature (新機能), improvement (改善), maintenance (保守), tech debt (技術的負債), or incident (障害対応). It comes from the board's "Work type" field and, when an item has none, from its pull request's Conventional Commits type, by the rule the node documents (the node computes the split, not the screen). What neither classifies is *unclassified* (未分類); it is shown as such and never guessed. The wire names are `newFeature`, `improvement`, `maintenance`, `debt`, `incident`.
- **Plan** (計画). What the org intends for a team (not a subscription's plan, the product tier of a funding source, which the costs screen calls a plan too), in `teams[].plan` of the org definition, signed by the root key: the *allocation* (割合), the share of the team's effort for each work type, whole numbers that sum to 100, and the *weekly budget* (週の予算), whole US dollars at API prices. A team may have none; the screen then compares nothing.
- **Effort** (労力). What a team spent, by work type: the token cost, API-priced as in a token usage report, served at `/api/orgs/<org>/effort`.

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
| person, for a participant; 人 for a participant or a count of them | participant (参加者) | A participant may be an agent. |
| device, for a push token | push token | A device is a signing key. |

"Host" is also fine as the verb of a node that runs a team's runtime (a *hosted team*), and in the `Host` header.

`scripts/check-words.sh` fails the build on the words above that a machine can tell apart from a good use (the Japanese ones in `web/src/i18n/ja.ts` and `docs/*.ja.md`, and the English ones in `web/src/i18n/en.ts`). The others (人 for a participant, 稟議書 for anything but the document itself) are left to review. Three phrases are let through because they are not about a node or a machine of ours: ホスト名 (the public host name), セルフホスト (Cloudflare's self-hosted application type), and Apple のサーバー (Apple's own servers).
