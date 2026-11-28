# What a node keeps

Everything a node writes, where, for how long, who can read it, and how to delete it; then its keys; then what leaves
the machine and to whom. It follows [protocol.md](protocol.md), and a change to one changes the other in the same pull
request.

The data directory is the one `--data-dir` names, else `$AGENCY_DIR`, else `~/.agency`. The node creates it readable by its own operating
system user only (`0700`), and sets the account key and the owner's numbers to `0600` as well. Nothing in it is encrypted
by the node: on a machine whose disk the
operating system encrypts (FileVault, LUKS, BitLocker), it is encrypted at rest with the rest of the disk. A backup of
the directory holds everything below except the root keys, which are kept apart (see "Keys").

"Personal data" marks what names or describes a person: a member's id and label, an email address, a device's label, the
text of a decision or a request.

## The data directory

| Path | What it holds | Personal data | Kept for | How to delete it |
|---|---|---|---|---|
| `format` | The number of the data directory's format, so a node of an older release refuses to start on a newer directory instead of misreading it | No | For the directory's life | Only with the rest of the data |
| `config.json` | The node's settings: its address, its account's id and label, the teams it runs (their `people` maps, and each runtime's command and `env`, which may hold a credential the runtime needs), funding sources (whose `usageFile` may name a file outside the data directory), the Access owner's email, push and Web Push settings | The account's label, the owner's email; a credential if a runtime's `env` holds one | Until changed | Edit the file |
| `account.key` | The account's private key: it signs reads, join requests, and the account's first device | No | For the node's life | Only with the node: it cannot be replaced (see "Keys") |
| `cli-device.key` | The private key of the node's own device (`agency decide`) | No | For the node's life | Remove the device from a client's Devices screen, then the file |
| `orgs/` | Each org's current signed definition (`<org>.json`), every version it took (`<org>.history/<version>.json`), and when it took a recovery's start (`<version>.recovery-start.json`) | Members' ids and labels, devices' labels, seats', departments' and external parties' labels | Forever: each version names the one before it, so none can be dropped without breaking the chain (`agency audit` reads them) | Removing a member leaves their name in older versions. Only removing the org's files removes it, and the node then no longer has the org |
| `replicas/` | The last view this node read of each team on another node (`<org>/<team>.json`): its requests, work items, people's states, thread messages, and when it was read | Request and thread texts, people's runtime names and states | Replaced at each read; dropped when the team leaves the org | Taken out when the team is; `agency sync` reads them again |
| `about/` | What each other node of an org says about itself (`<org>.json`): its release and capabilities | No | Read again every day | Rewritten as it is read |
| `decisions.json` | This account's decisions: when, the request's title, the text and choice, whom it answered for, which device signed it, and its state | Decision texts | The last 500 | Edit or remove the file |
| `applied.json` | The ids of the envelopes this node applied to a team it runs, with when, so a copy is not applied twice | No | 7 days and 5 minutes (an envelope's life), pruned as new ids are written | Remove the file (a copy sent within the window would then be applied again) |
| `outbox/`, `refused/` | Parcels for other nodes (decision envelopes, versions, device changes) until they are delivered, and the ones refused | Decision texts in envelopes | Until delivered or refused; refused ones until removed. An envelope past its 7 days is refused as expired | Remove the files |
| `invites/` | Each invite's record, under the SHA-256 of its code (never the code): org, place, role, expiry, and who used it | The account id that used it | Until removed; an invite expires after 7 days | Remove the files |
| `claims/`, `claims-by-seat/` | Each seat's claim link, under the SHA-256 of its code: the seat, the email it is for, its expiry, and once redeemed the device key and label sent with it | The email, the device's label | Until confirmed, refused or replaced; a link expires after 7 days | `agency claim-reject` drops a waiting one |
| `pending-root/` | Changes held for the root key's presence (joins, device changes, claim confirmations), and the version that signed each | The joiner's id and label, a device's label | 7 days, then dropped | `agency root-sign --refuse <handle>` |
| `waiting-joins/` | Joins this node asked another org's root to hold, with the handle to fetch the version | No | Until the join is signed or refused | Remove the file |
| `guests/` | Which sign-in email is which hosted member (`<sha256 of the email>.json`), and the requests each guest was already told of (`notified/`) | The email | Until the member is removed or its record is replaced | `agency member-remove` or `agency guest-remove` drops a member's sign-in records and its `notified/` list |
| `device-requests.json` | Devices that asked to be added and wait, with their pairing codes | Devices' labels | One hour; an expired request is no longer offered, and leaves the file the next time a request is saved | Remove the file |
| `push-tokens.json`, `web-push-subscriptions.json` | The iPhones' APNs tokens and the browsers' Web Push subscriptions, with their labels, devices, and last answers | Devices' labels | Until removed, or the push service says they are gone, or their device is removed | From the Devices screen, or remove the entries |
| `push-retries.json` | Notifications that failed for one destination and are tried again: the notice's key and the destination, never its words | No | At most a day, 1000 at most | Remove the file |
| `notified.json` | What the owner was already told of (keys only) | No | The newest 1000 of each kind | Remove the file (old news may then be told again) |
| `desk-refresh.json` | When the iPhones were last woken to read the desk, and what it held then: one snapshot | No | Replaced at each pass | Remove the file |
| `runtime-releases.json` | For each team this node runs, the release its runtime said it is (`describe`), or none, and when it was read | No | Rewritten whole; an entry stays until its team is read again | Remove the file: it is made again at the next runtime call |
| `clients-seen.json` | Which app versions called the node, and when, for the oldest version still in use | No | 32 versions per platform | Remove the file |
| `bound-readers.json` | The accounts whose node signs reads that name this node, so an older form of read from them is refused | Account ids | While they are members of an org this node has | Remove the file (older reads are then taken again) |
| `own-origins.json` | The origins this node has been configured with, so a team's place under an older one is still this node | No | The last 20 | Remove the file |
| `outcomes/` | The owner's numbers for each KPI (`<org>.jsonl`), and the days epics shipped (`<org>.ships.jsonl`); only on the owner's node | Who entered a number | Until removed: they are business data | Edit or remove the files |
| `usage/` | Each funding source's usage windows (`<id>.json`) as its collector last read them | No | Rewritten at each read | Remove the files |
| `metrics/` | How long each request and each call to another service took, one file per UTC day: route names and status classes, never ids, addresses, bodies or tokens | No | 30 days | Remove the files |
| `logs/` | The node's log (`node.jsonl` and closed files), and what the service manager caught (`serve.log`): finished sentences that name no token, endpoint, request body or query | Account ids, labels, request titles in some lines | By size and age: each file up to `maxSizeMib` (default 20), closed files `maxAgeDays` (default 14), at most `maxFiles` (default 50) | `agency logs` reads them; remove the files |
| `web-push-vapid.key` | The node's Web Push key, made the first time Web Push is on | No | For the node's life | Remove it: every browser must then subscribe again |

## Keys

| Key | Where it lives | Who holds it | Rotation | Losing it |
|---|---|---|---|---|
| Account key | `account.key` in the data directory | The node, for its one account | None: the account is the key | The node can no longer sign reads or join requests. Members of its orgs stay; start a new node with a new account and join again |
| Root key of an org | Outside the data directory: `$XDG_CONFIG_HOME/agency/root-keys/`, or `~/.config/agency/root-keys/` (files readable by the user only), or the macOS login keychain with user presence (`rootKeys: keychain`) | The root's node | `agency root-rotate --org <org>` | Nobody can change the org any more, unless it has a recovery key |
| Recovery key of an org | Nowhere on a machine: 24 words the owner wrote down (`agency root-recovery`) | The owner, on paper | `agency root-recovery` again replaces it | The root key still works; without both, the org can't be changed |
| Device keys | On each device: non-extractable in the browser (IndexedDB) and the Secure Enclave on an iPhone; `cli-device.key` for the node's own | Each device | Add a new device, remove the old | Remove the device from another device; its key is revoked in the next version |
| Web Push key (VAPID) | `web-push-vapid.key` in the data directory | The node | Remove it and restart | Browsers must subscribe again |
| APNs provider key | The `.p8` file `push.keyPath` names, outside the data directory and never copied into it | The owner | Make a new one in Apple's developer account | Notifications to iPhones stop until a new one is set |
| Access (sign-in) | Nothing secret: the node verifies the provider's signed tokens with its public keys, fetched from the provider | The identity provider | The provider's | — |

## What leaves the machine, and to whom

- **Other nodes of an org**, over HTTPS, signed: the org's versions, this node's decisions for teams on them, device changes, join requests, and signed reads of their teams' views, usage, activity and numbers. A team this node runs answers those reads to the org's members, and to answerers only what is addressed to them.
- **Push services** (APNs; for browsers, the push service of each subscription): a notice's kind, its badge, and, unless `hideTitles` is set, a team's label and a request's or decision's title. Web Push messages are encrypted to the browser. The contact in the Web Push header names this node to the push services (`webPush.subject`).
- **GitHub** (or the host a team's delivery names), with the login the machine has: read-only queries about the team's repositories, for delivery numbers.
- **The identity provider** (Cloudflare Access): the node fetches its public keys. The provider sees the sign-ins.
- **The AI services of the funding sources** whose usage the node collects (`funding[].collector`): it asks each for the login's usage windows, with that login's own credentials, at most every 5 minutes. It never sends them anything of the org.
- **Nothing else.** The node sends no telemetry of its own anywhere: the numbers in `metrics/` are read only where the node runs, or by a scrape the owner sets up.
