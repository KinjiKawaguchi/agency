# Installing a node

This guide installs an Agency node on a Mac from source, runs it as a launchd agent, and adds your first device.
It then shows how to make the node reachable from other machines behind Cloudflare Access.
Linux and systemd come later; for now this guide covers macOS only.

The Japanese version is [install.ja.md](install.ja.md). It has the same sections; this English version is the source.

The examples use the account `alice`, the org `acme`, the team `app`, and the host name `agency.example.com`; use your own.
Ids are lowercase letters, digits, and hyphens (`[a-z0-9][a-z0-9-]{0,39}`).

## Requirements

- macOS on Apple silicon or Intel.
- Xcode Command Line Tools: `xcode-select --install`.
- Rust, stable, 1.87 or later, from [rustup](https://rustup.rs). Check with `rustc --version`.
- Node.js 24 with npm, to build the web client. Check with `node --version`.
- The team's harness and the tools it uses (see [The team runtime](#the-team-runtime)).

Keep the repository, the data directory, and the team's state directory outside `~/Documents`, `~/Desktop`, and `~/Downloads`.
macOS keeps launchd agents out of those folders unless they get Full Disk Access.

## Build from source

Build the web client first: the node embeds it when it is built.

```sh
git clone <this repository's URL> ~/src/agency
cd ~/src/agency/web
npm ci
npm run build
cd ..
cargo build --release -p agency-node
```

Install the binary at a path that a rebuild or `cargo clean` does not touch.
Copy it under a new name and then rename it, so that a running node never has its binary rewritten in place:

```sh
mkdir -p ~/.local/bin
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
~/.local/bin/agency --help
```

The rest of this guide calls it `agency`: put `~/.local/bin` on your `PATH`, or write the full path.
Without `--dir`, `agency` keeps its data in `$AGENCY_DIR`, or in `~/.agency` when that is not set.

## A local-only node

A local-only node runs on one Mac by itself.
It listens on `127.0.0.1` only, so the web client works only on that Mac, and the node lets in only requests from that Mac.
No other node reaches it and it reaches no other node: there is no tunnel, no Cloudflare, and no push notifications.
Everything it stores stays in its data directory.

### Create the node, the org, and the team

```sh
agency init --account alice --label "Alice" \
  --url http://127.0.0.1:4400 --listen 127.0.0.1:4400 \
  --scrum "$(command -v scrum)"
agency org-create --org acme --label "Acme"
agency team-add --org acme --team app --label "App" --state-dir ~/teams/app
agency status
```

- `--url` is where other nodes would reach this node. On a local-only node it is its own loopback address, with the same port as `--listen`.
- `--scrum` is the harness's command. Give its full path (as above), or a bare name that the node looks up on its `PATH`.
- `--state-dir` is the team's state directory for the harness. A relative path is stored as an absolute one.
- `org-create` makes this node's account the org's root key. `team-add` without `--node` runs the team on this node, owned by this node's account.

`agency status` should show the account, `Org acme (Acme), version 2, root key`, and `Team on this node: acme/app (owner alice) runs <scrum> runtime`.

The data directory now holds `account.key`, the org's root key.
Anyone who can read that file can act as the org's root.
Keep it on this Mac, and out of anything that syncs or shares files.

### Run it with launchd

`scripts/launchd-plist.sh` writes the agent's file.
The agent gets the `PATH` of the shell you run the script from, so the team runtime finds the same tools as your terminal does. Run it from a terminal where `scrum` and the harness's tools work.

```sh
scripts/launchd-plist.sh --agency ~/.local/bin/agency --dir ~/.agency \
  --out ~/Library/LaunchAgents/local.agency.node.plist
plutil -lint ~/Library/LaunchAgents/local.agency.node.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/local.agency.node.plist
```

The agent starts `agency serve` at login and starts it again if it stops.
The node's output goes to `~/.agency/logs/serve.log`.

| To | Run |
|---|---|
| See whether it runs | `launchctl print gui/$(id -u)/local.agency.node \| grep state` |
| Restart it | `launchctl kickstart -k gui/$(id -u)/local.agency.node` |
| Stop it until you load it again | `launchctl bootout gui/$(id -u)/local.agency.node` |

If your `PATH` changes (for example, the harness moves), run the script again, then `bootout` and `bootstrap`.

### Add your browser as the first device

Decisions are signed by your devices, never by the node.
Your first device is added on the node's terminal, with the node's account key.

1. Open `http://127.0.0.1:4400/` in your browser. Always use this address: to the browser, `http://localhost:4400/` is a different site, and it keeps a different key there.
2. Open **Devices** and choose **Add this device**. The page shows a pairing code, such as `8945-bea8-e6d8`.
3. In a terminal, run `agency device-add`. It asks `Add "<name>" with pairing code 8945-bea8-e6d8?`. Answer `y` only if every character matches the code in the browser.
4. The page says that the device is added.

To decide from the terminal as well (`agency decide`), add the CLI device with `agency device-add --cli`.
If the browser is already added, the CLI device asks to be added instead: confirm it under **Devices** in the browser, after comparing the code.

### Check that it works

```sh
agency status
curl -s http://127.0.0.1:4400/api/me
curl -s -w ' %{http_code}\n' http://127.0.0.1:4400/api/orgs/acme/teams/app/view | tail -c 200
tail -n 20 ~/.agency/logs/serve.log
```

- `/api/me` lists the account, and the org with the team `app` and `"location":"local"`.
- The team's view ends in `200`. With `503`, the answer's `error` says why the node could not run the team runtime (see [Troubleshooting](#troubleshooting)).
- In the browser, **Decisions** shows what the team waits on you for. Answer one, then check that **Recently decided** marks it **Applied** and that the team received it.

To check the team runtime by itself, run it the way the node does:

```sh
echo '{"runtime":"agency-runtime/1","org":"acme","team":"app","op":"describe"}' \
  | SCRUM_STATE_DIR=~/teams/app scrum runtime
```

It must print one JSON line whose `result.runtimes` contains `agency-runtime/1`.

## Reaching a node from outside with Cloudflare Access

To use the node from a phone or another machine, or to let other nodes reach it, put it behind a Cloudflare Tunnel and Cloudflare Access.
The node keeps listening on `127.0.0.1`; the tunnel connects to it from the same Mac.
Clients sign in to Access, and the node checks Access's token on every request. Other nodes do not sign in: they sign each request with their keys, so their routes bypass Access.

### Change the node's address

Other nodes reach a node at its `url`, and each org records where its teams run.
When the node gets its public host name, set `"url": "https://agency.example.com"` in `~/.agency/config.json`, restart the node, and move each of its teams (on the node that holds the org's root key, which is this node for the orgs it created):

```sh
agency team-move --org acme --team app --node https://agency.example.com
```

Until then, the node still runs and serves its teams, and its log says which teams other nodes look for elsewhere.
A node that only you use from your own devices, and that no other node reaches, can keep its loopback `url`.

### Cloudflare Tunnel

1. Install `cloudflared` (`brew install cloudflared`).
2. In Cloudflare Zero Trust, create a tunnel, and give it a public host name, `agency.example.com`, whose service is `http://127.0.0.1:4400`.
3. Run `cloudflared` on the Mac as Cloudflare's instructions for the tunnel say, as a service that starts at boot.

Requests through the tunnel carry Cloudflare's headers, so the node treats them as coming from outside and asks for an Access sign-in, even though they reach it from `127.0.0.1`.

### The Access application

In Cloudflare Zero Trust, add a self-hosted Access application for `agency.example.com`, with a policy that allows only you (for example, your email address).
Note two values from it for the node's setting:

- The application's audience (AUD) tag, 64 hex characters.
- Your team domain, `<team name>.cloudflareaccess.com`.

### Paths that bypass Access

Two paths must reach the node without an Access sign-in. Add an Access application (or policy) for each path with the action Bypass:

- `agency.example.com/fed/*`: the federation routes. Other nodes cannot sign in to Access; the node checks their signatures instead (see "Federation routes" in [protocol.md](protocol.md)).
- `agency.example.com/.well-known/apple-app-site-association`: Apple's servers fetch it without signing in, for the iPhone app's sign-in. Needed only with `appId` below.

Bypass nothing else. The node refuses every other request from outside that has no valid Access token.

### The access setting

Add `access` to `~/.agency/config.json` and restart the node:

```json
"access": {
  "publicHost": "agency.example.com",
  "teamDomain": "acme.cloudflareaccess.com",
  "audience": "<64 hex characters>",
  "ownerEmail": "alice@example.com"
}
```

- `ownerEmail` is the email you sign in to Access with. Requests from other signed-in emails may read, but only `ownerEmail` may write (decide, add devices).
- For the iPhone app's sign-in outside its web view, add `"appId": "ABCDE12345.com.example.agency"` (`<team id>.<bundle id>`) and the second bypass above. See [apple.md](apple.md).
- To notify your iPhone of new requests, add a `push` setting with the app's APNs key; see the README.

A malformed setting is reported at start (see [Troubleshooting](#troubleshooting)); the node then refuses every request through Cloudflare.
"Who is let in" in [protocol.md](protocol.md) says exactly what the node checks.

## Adding a phone or a browser

Each browser and phone that decides for you has its own key, which it makes and keeps; the key never leaves it.

1. On the new device, open the web client: `http://127.0.0.1:4400/` on the node's Mac, or `https://agency.example.com/` after signing in to Access. The iPhone app opens the same page ([apple.md](apple.md)).
2. Open **Devices** and choose **Add this device**. The page shows its pairing code.
3. Confirm the code in one of these places:
   - on a device you have already added: open **Devices**, find the request, and confirm it if the codes match;
   - on the node's terminal: `agency device-add`. Without any device yet, it signs with the node's account key; afterwards, with the CLI device once that is added.

A request waits for an hour, and the node keeps at most 10.
A browser makes a key only on `https` or on the node's own Mac (`127.0.0.1`); elsewhere the page says it cannot.

## AI subscriptions and their usage

The overview lists the node's funding sources with how much of each subscription's limits is used.
A source is one person's own subscription: the node reads it with that person's own login and never shares it with another account.
The node can read the usage of a Claude subscription itself, as many as the machine has logins for.

### Several Claude logins on one machine

Claude Code keeps one login for each configuration directory, the one it runs with as `CLAUDE_CONFIG_DIR`. To add a login, give it a directory of its own and log in once there:

```sh
CLAUDE_CONFIG_DIR=$HOME/.claude-accounts/personal-2 claude
```

Then run `/login` in it. Start Claude Code with the same `CLAUDE_CONFIG_DIR` whenever you want to use that login.

Where Claude Code keeps the login:

| Machine | Where |
|---|---|
| macOS | The Keychain item `Claude Code-credentials-<8 hex characters>`: the first 8 hex characters of the SHA-256 of the directory's text, as it is set in `CLAUDE_CONFIG_DIR`. Without `CLAUDE_CONFIG_DIR` (the login in `~/.claude`) the item is `Claude Code-credentials`. When the Keychain refuses the write, Claude Code uses the file below. |
| Linux | `<directory>/.credentials.json`, mode `0600` (`~/.claude/.credentials.json` without `CLAUDE_CONFIG_DIR`). |

The directory's text is what counts: `/home/alice/.claude-work/` with a trailing slash is another item than `/home/alice/.claude-work`.
Write the directory in the node's configuration exactly as you set `CLAUDE_CONFIG_DIR`, as an absolute path.

### Reading the usage

List the sources in `~/.agency/config.json`, with no secret in it, and restart the node:

```json
"funding": [
  { "id": "personal-1", "label": "Claude (personal 1)", "kind": "subscription",
    "collector": { "type": "claude" } },
  { "id": "personal-2", "label": "Claude (personal 2)", "kind": "subscription",
    "collector": { "type": "claude", "configDir": "/Users/alice/.claude-accounts/personal-2" } }
]
```

- `collector` makes the node read the source itself. Without `configDir` it reads the login in `~/.claude`.
- The usage goes to `usage/<id>.json` in the data directory, or to `usageFile` when the source has one. A source with only a `usageFile` is kept by another tool, as before.
- For each source the node reads the login's access token when it needs it, asks `https://api.anthropic.com/api/oauth/usage` for the 5-hour and 7-day windows, and writes only those two windows. It keeps no token and writes none to a file or the log.
- It asks each source at most every 5 minutes. After a failure it waits twice as long each time, up to an hour, and longer when the service asks for it. A failed read leaves the last file in place, and the overview shows when the file was last written.
- It never renews a token: that would replace the refresh token Claude Code holds for the login. An access token lasts some hours and Claude Code renews it when it runs with that login, so a login nobody has used for a day reads as expired until it is used again.
- On macOS, the first read of each Keychain item asks you to allow `security` to use it. Choose **Always Allow**. The node waits 20 seconds for the answer and tries again later.

## Updating a node

```sh
cd ~/src/agency
git pull
(cd web && npm ci && npm run build)
cargo build --release -p agency-node
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
launchctl kickstart -k gui/$(id -u)/local.agency.node
```

The data directory is not touched.
The node picks a new write token each time it starts, so open pages reload once before their next write.

## Troubleshooting

### Log lines

The node writes these to `~/.agency/logs/serve.log`:

| Line | Meaning |
|---|---|
| `Node running: <url> (listening on <address>)` and `Web client: …` | The node started. |
| `agency: cannot listen on 127.0.0.1:4400: Address already in use` | Another process holds the port; launchd tries again every 10 seconds. `lsof -nP -iTCP:4400 -sTCP:LISTEN` shows which. |
| `agency: team acme/app runs on this node (…), but the org places it at …` | The node's `url` changed. Run the `agency team-move` the line gives. |
| `agency: the access setting is malformed (…)` | Fix `access` in `config.json`. Until then, every request through Cloudflare is refused. |
| `agency: the access appId is malformed (…)` | Fix `appId`. The rest of `access` still works; the app sign-in does not. |
| `agency: refused an Access request (no Access token)` | A request came through Cloudflare without an Access sign-in: a bypass covers more than the two paths, or Access is not in front of the host name. |
| `agency: refused an Access request (wrong audience)` | `audience` is not the application's AUD tag. |
| `agency: refused an Access request (wrong issuer)` | `teamDomain` is not your team domain. |
| `agency: refused an Access request (expired)` or `(not valid yet)` | The sign-in expired, or this Mac's clock is off. |
| `agency: refused an Access request (cannot fetch the Access certs: …)`, `(unknown signing key)`, or `(the signature does not match)` | The node cannot get Access's keys from `teamDomain`, or the token was not made for this team domain. |
| `agency: push notifications are off: …` | The `push` key cannot be read. Nothing is sent. |
| `agency: a push notification was not sent: …` | APNs refused one; it is tried again 30 seconds later. |
| `agency: usage of "<id>" could not be read: …; trying again in <n> minutes` | The collector of that funding source failed; the text says why (no login found, expired token, refused token, the service unavailable). The last usage file stays. Log in again in that source's configuration directory, or use Claude Code with it, and the next read succeeds. `agency: usage of "<id>" is read again` follows. |
| `agency: re-reading the replicas failed: …`, `agency: checking for new requests failed: …`, or `agency: retrying the outbox failed: …` | The node could not write to its data directory (for example, the disk is full). Another node that cannot be reached is not logged: `agency status` lists what waits in the outbox, and the web client shows when the replica of a team on another node was last read. |
| `agency: <message>`, and the node stops | It could not start, for example because `config.json` does not parse. launchd tries again every 10 seconds. |

### Answers from the node

Some problems show only in the node's answers, in the browser or with `curl`, not in the log:

| Answer | Meaning |
|---|---|
| `503` `cannot start the team runtime: No such file or directory` | The team's runtime command is not at the path in `config.json` (`teams[].runtime.command`). |
| `503` `the team runtime did not answer within 20 seconds` | The runtime hangs, or is too slow. |
| `503` `the team runtime did not write an agency-runtime/1 response (…): …` | The runtime failed; the end of the answer is what it wrote to stderr. Often a tool missing from the launchd `PATH`. |
| `421` `wrong host` | The page was opened at an address other than `127.0.0.1:<port>` or `localhost:<port>`. |
| `403` `the Client API is local only` | A request from another machine reached the node without Cloudflare. |
| `403` `missing or wrong write token` | The page is older than the node's last start. Reload it. |
| `403` `only the node's owner may decide` | Signed in to Access with an email other than `ownerEmail`. |

## The team runtime

The node never reads the harness's files. For each team, it runs the team's runtime command (`<scrum> runtime`, with `SCRUM_STATE_DIR` set to the team's state directory) and talks to it in agency-runtime/1; see [protocol.md](protocol.md#agency-runtime1).
Set up the harness by its own instructions. What the node needs from it:

- A command that answers agency-runtime/1: `describe`, `view`, and `decide`, and `tokenUsage` for the costs screen. Each call must answer within 20 seconds.
- The team's state directory, the same one as `team-add --state-dir`.
- Every tool the runtime calls, on the `PATH` written into the launchd agent.
- Requests for you leave `to` out or set it to `owner`. In an org where you are the only member, every request is yours. A participant waiting on you has `waitingOn` set to `{ "kind": "person", "person": "owner" }`.

While it runs, the node asks the runtime for the team's view every 30 seconds and each time a page reads the team, so `view` should be cheap.
