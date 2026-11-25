# Agency

A node for running organizations in which AI agents and people work together, across many machines.

- Each team of agents runs in a harness on its own machine (your laptop, a lab server, a work PC).
- From their own node, a person answers what needs deciding (questions, proposals, pending decisions) in every org they belong to.
- An answer becomes an envelope signed with the decider's key and is delivered to the team's node. If that node is asleep, the envelope waits in the outbox and is delivered when it wakes up (store-and-forward).

There is no central server.
The only central thing is authority (the org's root key); the data is split across machines, and every piece of data has exactly one writer.
The whole protocol is in [docs/protocol.md](docs/protocol.md).

## Units

| Kind | Unit | Writer |
|---|---|---|
| Execution (activity, requests, decisions, board) | Team (one harness) | The node the harness runs on |
| Governance (departments, people, where teams run, revoked keys) | Org | The org's root key |
| Person (key, memberships) | Account | That person |

## Contents

```
crates/agency-proto    The protocol: canonical JSON, Ed25519 signatures, envelopes, org definition versions, invites and joining
crates/agency-core     Domain, use cases, and ports; no async runtime, network, filesystem, wall clock, or randomness
crates/agency-adapters Port adapters: data directory, HTTP in and out, team runtime commands, system clock, test doubles
crates/agency-node     Wiring and the `agency` CLI
docs/protocol.md       The protocol specification
testdata/vectors.json  Language-independent test vectors (a key from a fixed seed, canonical JSON, a signature)
```

## Build and test

```sh
cargo build --release
cargo test
cargo clippy --all-targets
```

## Usage

Create an org, add a team, and invite people (on the root key's node):

```sh
agency --dir ~/.agency init --account alice --label Alice --url https://alice.example --scrum /path/to/scrum
agency --dir ~/.agency org-create --org acme --label Acme
agency --dir ~/.agency team-add --org acme --team app --label app --state-dir ~/.claude/scrum
agency --dir ~/.agency invite --org acme --role member
agency --dir ~/.agency serve
```

Join from an invite link and answer (on the invited person's node):

```sh
agency --dir ~/.agency init --account bob --label Bob --url https://bob.example
agency --dir ~/.agency join 'agency:join?node=...&org=acme&fp=...&code=...'
agency --dir ~/.agency sync
agency --dir ~/.agency device-add --cli
agency --dir ~/.agency decide --org acme --team app --request r-20260101-000000-abcd --choice a
```

The CLI signs decisions with a device key, never with the node's account key.
`device-add --cli` adds this terminal's own device, which `decide` signs with.
To add a phone or a browser, ask to add it from the web client.
For your first device, run `agency device-add` on the node's terminal, check that the pairing code matches, and confirm.
After that, add more devices from one you already have. Once the CLI device is added, `agency device-add` on the terminal adds them too.
Add the CLI device first (`device-add --cli`), so you can add the rest from the terminal.

If a node's URL changes (for example, when it moves behind a tunnel), set `url` in its `config.json`, then, on the root key's node, move each of its teams to the new origin:

```sh
agency --dir ~/.agency team-move --org acme --team app --node https://agency.example.com
```

Until then, the node still runs and serves its teams, and `agency status` and `agency serve` say which teams other nodes look for elsewhere.

An invite link carries the fingerprint of the org's root key.
Before joining, the node checks that the key in the org definition the inviting node returns matches that fingerprint, and does not send a join request if it does not.

## Web client

`agency serve` also serves the web client and its Client API at `http://127.0.0.1:<port>/`. Build the client before the node, so the node embeds it: `npm run build` in `web/`, then `cargo build`. Without a build, the node serves a page that says so.

To open it from elsewhere, put the node behind a Cloudflare Tunnel with Cloudflare Access, and add to the node's `config.json`:

```json
"access": { "publicHost": "agency.example.com", "teamDomain": "acme.cloudflareaccess.com", "audience": "<64 hex characters>", "ownerEmail": "alice@example.com" }
```

The Access policy must bypass `/fed/*` so other nodes can reach the federation routes.

To notify the owner's iPhone of new requests, add the APNs key Apple issued for the app (keep the `.p8` file outside the repository):

```json
"push": { "keyPath": "/path/to/AuthKey_ABC123DEFG.p8", "keyId": "ABC123DEFG", "teamId": "TEAM123456", "bundleId": "com.example.agency" }
``` See "Client API" in docs/protocol.md for what is checked.

## iPhone app

`apple/` holds a thin iPhone app that opens the home node in a web view, for TestFlight. See [docs/apple.md](docs/apple.md).

## Team runtimes

The node never reads or writes a harness's files.
For each team it hosts, it runs the team's runtime command and speaks agency-runtime/1 (`describe`, `view`, `decide`, and the optional `tokenUsage`) as JSON over stdin and stdout; see docs/protocol.md.
`team-add --state-dir` configures the scrum harness's runtime, `<scrum> runtime` with `SCRUM_STATE_DIR`, owned by this node's account unless `--owner` names another org person.

## Not yet

- Department heads signing their own department's definition: today only the org's root key signs.
- Flows for a lost account key, a person who leaves, or a person who loses every device (a lost device is removed from another of their devices).

## License

Either MIT or Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
