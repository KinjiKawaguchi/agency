# Security

Who is trusted for what, what each can and cannot do, what a node never does, and what is known not to be covered. It is short on purpose: [protocol.md](protocol.md) is the specification, [data.md](data.md) says what a node keeps and where, and [compatibility.md](compatibility.md) says how the rules change. To report a vulnerability, see [SECURITY.md](../SECURITY.md).

## What is protected

- **A decision is a member's.** What an org decides is signed by a device key that the member holds. No node, and no team's runtime, can make a decision up.
- **The org's definition is the root key's.** Who is a member, which devices they have, where each team runs, and which keys are revoked change only by a version that the org's root key signs.
- **What a member reads stays among members.** A request is read by the org's members, and an answerer reads only what is addressed to it. A guest reads only its own requests.
- **What belongs to the owner stays on the owner's node:** the org's numbers, the node's measurements, and the node's keys.

## Who is trusted for what

| Party | Trusted to | Cannot |
|---|---|---|
| **The org's root key** (held by the root's node, apart from its data directory; on a Mac it can sit in the keychain behind the owner's presence) | Alone, sign the org's versions: members, seats, departments, teams, revoked keys, and the root itself | Sign a decision |
| **The recovery key** (24 words kept on paper by the owner; on no machine) | Move the root key, after a wait of 72 hours in which the root key can cancel it | Do anything else |
| **A member's device** (a browser, an iPhone, the CLI) | Sign that member's decisions and changes to that member's devices | Act for another member. The key of a browser or an iPhone cannot be extracted from the device; the CLI's is a file (`cli-device.key`) |
| **A member's home node** | Hold the account key, keep replicas of what the account sees, and serve the account's clients | Sign the decisions of its owner's browsers and iPhone: those are signed on the devices. It can read all the account reads, and sign reads as the account |
| **A team's node, and the team's runtime** | Run the team: say which requests exist, apply the decisions sent to it, and report the team's view and numbers | Sign for anyone. It *can* invent a request, ignore or delay a decision, and report numbers that are false: what a team says is what its node says, and the signatures show who decided, not that the request was real |
| **Another node of the org** | Answer signed reads of the teams it runs, and relay what is signed | Do anything a signature does not cover. It reads what its members' signed reads allow, and it sees the decisions sent to its teams |
| **The identity provider** (Cloudflare Access today) | Say who signs in to the Client API from outside: the owner, and each guest | Decide. A person who signs in as the owner can read what the owner reads, and can ask to add a device, which a device already listed must approve after comparing pairing codes |
| **The machine and its operating system user** | Keep the data directory (readable by that user only) and the account key | Be protected from a process of the same user: on a node with no `access` setting, a local request is the owner |
| **Push services, GitHub, the AI services of funding sources** | Carry a notice, answer read-only queries, report usage | See an org's decisions. A notice holds its kind, its badge and, unless `hideTitles` is set, a title |
| **The release process** | Build what is tagged: the builds and the lists of materials, signed | Build what is not tagged. macOS builds are made on a Mac and are not signed ([releasing.md](releasing.md)) |

## What a node never does

- Sign an envelope. A node's account key signs reads, join requests, and the account's first device in an org, never a decision. The CLI on the node's terminal is a client, with a device key of its own.
- Trust a position in the network. The federation routes are checked by signatures, the Client API by a valid sign-in or a local request, and a request that fails before it is authenticated gets the same `404`, so that it cannot tell a stranger what the node holds.
- Follow a redirect in an answer to its own requests, since a redirect would carry its signed requests elsewhere.
- Apply a decision twice, or one that is older than 7 days.
- Write a token, a key or a request body in its log, or send anything of its own to anyone (it has no telemetry).

## Attacks that were considered

| Attack | What stops it |
|---|---|
| A decision made up, or by someone else | The signature is checked with a device key that the org's definition lists under a member; the account must be the request's addressee |
| A decision sent again | Each envelope's id is applied once; an envelope expires after 7 days and may be at most 5 minutes ahead |
| A signed read taken from the network and used again | A read is good for 5 minutes and is signed over its path. A read of the newer form also names the node it is for, so another node refuses it ([known limits](#known-limits) say what stays open) |
| An org definition changed on the way, or by a node | Every version is signed by the root key and names the one before it. A node refuses a version it cannot read and check |
| A page of another site that uses the owner's session | A write needs a token that only the served page holds, a JSON content type, and, through the provider, the owner's email. The page's headers allow only its own origin to run scripts or connect, and forbid framing |
| A request to the Client API with no sign-in | It is let in only when local or with a valid provider token for a known email. Nothing is admitted by default |
| A path made to be read differently by a layer in front of the node | A path with an encoded separator, a backslash or a dot segment is refused on every route |
| A lost or stolen device | Removing the device puts its key on the org's revoked list, so it can never sign again, under any id |
| A root key that is lost | The recovery key moves the root after a wait of 72 hours, which the root key can cancel |
| An invite or a claim link used again, or guessed | A code is 16 random bytes, kept only as its SHA-256, and it works once and expires after 7 days |
| A change that arrives in a release | Actions are pinned to a commit, dependencies wait before an update, builds use the lock files, and a release has lists of its materials and signed checksums |

## Known limits

These are known, and some are decisions of the design. A report that finds more is welcome.

- **A signed read of the older form can be used again** until the node requires the newer form (`federation.requireReadAudience`) or has seen its account use it. The newer form is used only toward nodes that say they take it.
- **One node can hold back the org's versions.** The root signs a version that uses a new field only when every node of the org's teams says it can read it. A node that never does stops those versions, for as long as it runs a team of the org. This is on purpose: signing over it would cut it off without anyone being told.
- **A stolen root key is not undone by the recovery key.** Whoever holds a copy can sign versions, rotate the root, and cancel a recovery, since the recovery's wait exists for the root key to cancel it. The owner answers a theft with `agency root-rotate` from their own copy, at once; the first to rotate holds the org. The key is kept apart from the node's data, and on a Mac it can sit behind the owner's presence.
- **A team's node is trusted for the team's content.** It can invent requests, ignore decisions, and report false numbers. Nothing in a member's signature stops that.
- **There is no role between a member and the root key**, and **no way to hide a team from a member**: every member reads every team of the org.
- **The account's first device in an org is added with the account key**, at the node's terminal. Whoever controls that key can add a device of their own in an org where the member has none, and decide as that member there. Once a member has a device in an org, a later one needs a listed device's signature, after the person compares the pairing codes.
- **Names stay in the signed versions.** Each version lists the members' labels, and all of them are kept, since each names the one before it. Use labels that name a role where that matters.
- **A node encrypts nothing at rest.** It relies on the disk's encryption, and keeps its data directory readable by its own operating system user only.
- **A process of the node's own operating system user** can read what the node reads: the account key, and the data. With an `access` setting it cannot write as the owner through the Client API; with none it is the owner.
- **Floods** are for the network and the provider in front of the node to absorb. The federation routes answer early and say nothing, but a node is not built to withstand them.
- **macOS builds are not signed or notarized**, and their checksums are not in the signed list of a release, since they are made on a Mac after the release is tagged.
