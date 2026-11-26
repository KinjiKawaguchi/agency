# Installing a node

This guide installs an Agency node on a Mac or a Linux machine from source or from a release build, runs it as a launchd agent or a systemd unit, and adds your first device.
It then shows how to make the node reachable from other machines behind Cloudflare Access.
It covers macOS (launchd) and Linux (systemd); a step that differs says which.

The Japanese version is [install.ja.md](install.ja.md). It has the same sections; this English version is the source.

The examples use the account `alice`, the org `acme`, the team `app`, and the host name `agency.example.com`; use your own.
Ids are lowercase letters, digits, and hyphens (`[a-z0-9][a-z0-9-]{0,39}`).

## Requirements

- macOS on Apple silicon or Intel, or Linux on x86_64 or arm64 with glibc 2.35 or later, and systemd to run the node as a service.
- macOS: Xcode Command Line Tools: `xcode-select --install`. Linux: a C compiler and `curl`, for example `sudo apt install build-essential curl`.
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

Install the binary at a path that a rebuild or `cargo clean` does not touch. Cargo builds it in `target/release/` of the repository, or in `$CARGO_TARGET_DIR/release/` when that is set.
Copy it under a new name and then rename it, so that a running node never has its binary rewritten in place:

```sh
mkdir -p ~/.local/bin
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
~/.local/bin/agency --help
```

The rest of this guide calls it `agency`: put `~/.local/bin` on your `PATH`, or write the full path.
Without `--dir`, `agency` keeps its data in `$AGENCY_DIR`, or in `~/.agency` when that is not set.

### A release build

Instead of building from source, you can use a release build: for each release, the Releases page of the repository has a tarball for macOS on Apple silicon (`arm64`) and on Intel (`x86_64`), and for Linux on `x86_64` and on `arm64` (`uname -m` says `x86_64` or, on Linux, `aarch64` for arm64). A Linux build runs on a system with glibc 2.35 or later. [releasing.md](releasing.md) says how releases are made.

1. Download the tarball for your machine and its `.sha256` from the release. With the GitHub CLI: `gh release download <tag> --repo <this repository> --pattern 'agency-*-<os>-<arch>.tar.gz*'`, where `<os>` is `macos` or `linux`. A build that is not of a release is a workflow artifact of the **release** workflow in the **Actions** tab (named `agency-<version>-<commit>-linux-<arch>`).
2. Check and unpack it, and install the binary the way the previous section does:

```sh
shasum -a 256 -c agency-*.tar.gz.sha256   # sha256sum -c on Linux
tar xzf agency-*.tar.gz
mkdir -p ~/.local/bin
cp agency-*/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
```

The tarball also holds the script that runs the node as a service (`launchd-plist.sh` and `deploy-macos.sh` on macOS, `systemd-unit.sh` on Linux; see [Run it with launchd](#run-it-with-launchd) and [Run it with systemd](#run-it-with-systemd-linux)), these guides, and the licenses. The binary already holds the web client.

The macOS builds are not signed and not notarized. A file downloaded with a browser is quarantined by macOS and will not open; remove the mark with `xattr -d com.apple.quarantine ~/.local/bin/agency` (`gh release download` and `curl` do not add it).

## A local-only node

A local-only node runs on one machine by itself.
It listens on `127.0.0.1` only, so the web client works only on that machine, and the node lets in only requests from that machine.
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
Keep it on this machine, and out of anything that syncs or shares files.

### Run it with launchd

`scripts/launchd-plist.sh` writes the agent's file.
The agent gets the `PATH` of the shell you run the script from, so the team runtime finds the same tools as your terminal does. Run it from a terminal where `scrum` and the harness's tools work.

The `PATH` is a copy, taken when you run the script, of that shell's. Every command the team runtime runs must be in a directory on it: for the scrum harness, `node` and `gh` as well as `scrum` (check with `command -v scrum node gh` in that terminal). A tool installed or moved afterwards, or a `PATH` that your terminal sets in a file the script's shell did not read, is missing from the agent, and the runtime then fails with `spawnSync gh ENOENT` (`ENOENT` is "not found"); the node logs `a command the team runtime runs was not found` when it does. Run the script again from a terminal where they all work.

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

### Run it with systemd (Linux)

`scripts/systemd-unit.sh` writes the unit's file (the release tarball holds the script as `systemd-unit.sh`).
The unit gets the `PATH` of the shell you run the script from, so the team runtime finds the same tools as your terminal does: a systemd user service otherwise gets only a short default `PATH`.
Run it from a terminal where `scrum` and the harness's tools work. As for the launchd agent above, every command the team runtime runs (for the scrum harness, `node` and `gh`) must be in a directory on that `PATH`.

```sh
scripts/systemd-unit.sh --agency ~/.local/bin/agency --dir ~/.agency \
  --out ~/.config/systemd/user/agency-node.service
systemctl --user daemon-reload
systemctl --user enable --now agency-node
```

The unit starts `agency serve` when you log in (at boot, with lingering, below) and starts it again 10 seconds after it stops.
The node's output goes to `~/.agency/logs/serve.log`, and `journalctl --user -u agency-node` shows when systemd started and stopped it.

| To | Run |
|---|---|
| See whether it runs | `systemctl --user status agency-node` |
| Restart it | `systemctl --user restart agency-node` |
| Stop it until you start it again | `systemctl --user disable --now agency-node` |

If your `PATH` changes (for example, the harness moves), run the script again, then `systemctl --user daemon-reload` and `systemctl --user restart agency-node`.

systemd runs your services only while you have a session. To have the node start when the machine boots, and keep running after you log out, turn on lingering once:

```sh
loginctl enable-linger "$USER"
loginctl show-user "$USER" --property=Linger
```

The second command must print `Linger=yes`. Without it, logging out of your last session stops the node.
Run `systemctl --user` in a login session (a terminal or an SSH login). From `su` or `sudo -u`, it cannot reach your services and says `Failed to connect to bus`.

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
The node keeps listening on `127.0.0.1`; the tunnel connects to it from the same machine.
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

1. Install `cloudflared` (`brew install cloudflared` on macOS; on Linux, Cloudflare's package for your distribution).
2. In Cloudflare Zero Trust, create a tunnel, and give it a public host name, `agency.example.com`, whose service is `http://127.0.0.1:4400`.
3. Run `cloudflared` on the machine as Cloudflare's instructions for the tunnel say, as a service that starts at boot.

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
- Once `access` is set, `http://127.0.0.1:4400/` on the node's Mac is for reading only: decide and add devices at `https://agency.example.com/`, signed in to Access, even on that Mac. This keeps other programs on the Mac from writing as you.
- For the iPhone app's sign-in outside its web view, add `"appId": "ABCDE12345.com.example.agency"` (`<team id>.<bundle id>`) and the second bypass above. See [apple.md](apple.md).
- To notify your iPhone of new requests, devices waiting to be added, and refused decisions, add a `push` setting with the app's APNs key; see the README. Add `"hideTitles": true` to it to send no title or label to Apple.
- To notify your browsers too (Chrome, Edge, Firefox, and Safari on a Mac or in the installed app, on Android, Windows, Linux, and macOS; not a browser on an iPhone, which has its own app), add `"webPush": {}` to `config.json` and restart. The node makes its own key in the data directory (`web-push-vapid.key`; keep it private like the account key) and the **Devices** screen then offers to turn notifications on for each browser. The notices go through the browser's push service (Google, Mozilla, Apple, or Microsoft), encrypted to the browser. `"subject"` (a `mailto:` or `https://` address the push service may reach you at; default the node's public host name, and without `access` the node's own address, which Apple's push service refuses: set it to a `mailto:` address then) and `"extraHosts"` (more push services to allow) are optional. To send no title or label to any of them, Apple included, put `"hideTitles": true` at the top of `config.json`; it hides them for the iPhone too.

A malformed setting is reported at start (see [Troubleshooting](#troubleshooting)); the node then refuses every request through Cloudflare.
"Who is let in" in [protocol.md](protocol.md) says exactly what the node checks.

## Adding a phone or a browser

Each browser and phone that decides for you has its own key, which it makes and keeps; the key never leaves it.

1. On the new device, open the web client: `http://127.0.0.1:4400/` on the node's machine, or `https://agency.example.com/` after signing in to Access. The iPhone app opens the same page ([apple.md](apple.md)).
2. Open **Devices** and choose **Add this device**. The page shows its pairing code.
3. Confirm the code in one of these places:
   - on a device you have already added: open **Devices**, find the request, and confirm it if the codes match;
   - on the node's terminal: `agency device-add`. Without any device yet, it signs with the node's account key; afterwards, with the CLI device once that is added.

A request waits for an hour, and the node keeps at most 10.
A browser makes a key only on `https` or on the node's own machine (`127.0.0.1`); elsewhere the page says it cannot.

## Installing the web client as an app

The web client can be installed as an app, so that Android, Windows, and Linux have one without a native build: it opens in a window of its own, with its own icon, and keeps the sign-in of the browser it was installed from. On the **Devices** screen, **Install Agency** appears where the browser offers it (Chrome and Edge, on Android, Windows, Linux, and Mac); in Safari on a Mac, choose File, then Add to Dock. The iPhone has its own app ([apple.md](apple.md)).

On the **Devices** screen, **Notifications in this browser** has two switches. The first shows what is new as a notification of the operating system while the page is hidden. The second, **When the app is closed**, is there only when the node has `webPush` (see below) and the browser has Web Push (not a browser on an iPhone, which has the app): the node then tells the browser through its push service even with no page open. The browser asks for the permission when a switch is clicked; one permission serves both. Each browser is subscribed separately, and each home node has its own subscription. If the node's key is ever made again, the screen says to subscribe again.

The installed app keeps the built files of the web client (`/assets/` and `/icons/`) so that it opens fast, and nothing else: no page, since a page carries the node's write token, and no answer of the API, which holds your org's data. When the node cannot be reached it shows a page that says so. When the sign-in expires it deletes what it kept.

Behind Cloudflare Access the node asks for the sign-in on every request, the manifest and the icons included, so the page fetches the manifest with the browser's credentials. No path needs to bypass Access for this. To check an install once the node is deployed:

1. Open `https://agency.example.com/` in Chrome (on Android and on a desktop), sign in, and open **Devices**: **Install Agency** appears.
2. Install it. The app opens in its own window on the desk, with the seal icon (not a blank one), already signed in.
3. Close the app, and sign out of Access (or wait for the session to end). Open the app again: it shows the Access sign-in, not an old page.
4. Turn the network off and open the app: it says the node cannot be reached.

If the icon is blank in step 2, the browser did not send the sign-in with the icon request; tell the team, and the icons will be inlined in the manifest.

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

To read a ChatGPT plan that you use through Codex, log in with ChatGPT (`codex login`) and list it as a source:

```json
{ "id": "codex", "label": "ChatGPT (Codex)", "kind": "subscription",
  "collector": { "type": "codex" } }
```

- `codexHome` names the `CODEX_HOME` of another login (default `~/.codex`). `command` is the path of `codex` when it is not on the `PATH` the node runs with.
- The node starts `codex app-server` and asks it for the plan's rate limits (`account/rateLimits/read`). The 5-hour and the weekly window go to the usage file, as for Claude, on the same schedule.
- Codex holds its own login, so the node reads no Codex token. Codex renews its login as it does for any run.
- A login that uses an API key has no plan windows; the log says so. Codex marks `app-server` as experimental; this was checked with Codex CLI 0.151.

### Choosing the subscription for new sessions

The program that launches agent sessions can ask the node which subscription a new session should run on. The order of `funding` is the order of preference, and each source with a `collector` can set a threshold on its 7-day window:

```json
{ "id": "personal-1", "label": "Claude (personal 1)", "kind": "subscription",
  "sevenDayThreshold": 90, "collector": { "type": "claude" } }
```

`sevenDayThreshold` is a percent from 1 to 100 (default 80).

```sh
agency funding pick --tool claude
```

prints one JSON object. `source` is the source to use, `env` is what to set for the session (`CLAUDE_CONFIG_DIR`, or `CODEX_HOME` for `--tool codex`; empty for the default login), and `reason` says why. `state` is:

| `state` | Meaning |
|---|---|
| `ok` | The first source whose 7-day window is below its threshold. |
| `unknown` | None is known to be below its threshold, and this source's usage cannot be read (the file is older than 30 minutes, or its window has reset). Unknown is not over. |
| `overThreshold` | Every readable source is past its threshold; this one has the most room before 100%. Sessions go on, and the overview should say so. |
| `none` | Every source has reached a limit (a window at 100). There is no `source`, and the command exits with 3. |

`agency funding list` prints the same sources in order, with their `env`, threshold, and where each stands now, for a program that needs to know every configuration directory and which logins are past their threshold. Each source has three flags, always present:

| Flag | True when |
|---|---|
| `overThreshold` | its 7-day window is at or past its threshold (also at 100) |
| `blocked` | a limit of the plan itself is reached: the 7-day window at 100, or the 5-hour window at 100 and not yet reset |
| `unknown` | its usage cannot be read now (no file, a file older than 30 minutes, or a window that has since reset); the other two flags are false then, since unknown is not past |

A program that moves running sessions off a login should do so only when that login is `overThreshold`, and not move them onto one that is `blocked` or `overThreshold`. `pick` carries the same flags for the source it chose. Neither command needs a running node or the account key, and neither prints a secret: `env` holds paths.

A session that is running keeps the source it started on; only new sessions move. The usage can be 5 minutes old, so a source can pass its threshold by what 5 minutes of work uses: leave room under 100.

Each source is one person's own subscription, and a node serves one account, so switching stays within one person's subscriptions. Never share a login with another person or pool logins across people: Claude's consumer terms forbid sharing an account. Switching is also not a way around a limit: a source at its limit is not picked until its window resets.

## Delivery metrics

The evaluation screen shows how fast each team ships: the pull requests it merged by type, how long they took from open to merge, how long its default branch's CI was red, reverts, and what its tokens cost for each merged pull request ([protocol.md](protocol.md#evaluation) defines every number). The node a team runs on works the numbers out from the team's repositories on GitHub, and other nodes read them from it.

To turn it on for a team, name its repositories (`owner/name`) in `teams[].delivery` of `~/.agency/config.json`, and restart the node:

```json
"teams": [
  { "org": "acme", "team": "app", "owner": "owner",
    "runtime": { "command": ["scrum", "runtime"] },
    "delivery": { "repos": ["acme/app", "acme/web"] } }
]
```

- The node reads GitHub through the machine's own `gh` login, read-only: it runs `gh api` for reads only (a GraphQL query for merged pull requests, the issues they close and when those moved to In Progress on the board, and a read of the workflow runs on the default branch). It holds no token, and the configuration has none. `gh` must be installed, on the `PATH` of the launchd agent or the systemd unit (see "Run it with launchd"), and logged in as the account that runs the node (`gh auth login`). `GH_HOST` and the rest of `gh`'s environment work as they do for `gh`.
- Prefer a login that can only read: `gh auth login --with-token` with a fine-grained token that has read access to the repositories' pull requests, issues, Actions, and metadata, and to the project (for the time an issue moved to In Progress).
- What GitHub said is kept for ten minutes for each team and period, so reading the screen does not spend the login's rate limit. Nothing is written to disk.
- A team with no `delivery`, or whose numbers cannot be read, is not an error: the screen lists it with the reason:

| Reason on the screen | What to do |
|---|---|
| no repositories are configured for this team | Add `delivery` to the team on the node it runs on. |
| the GitHub CLI (gh) is not installed on the team's node | Install it, and put its directory on the `PATH` of the agent or the unit. |
| the GitHub CLI (gh) on the team's node is not logged in | Run `gh auth login` as the account that runs that node. |
| GitHub's rate limit is used up for now | Wait; the node asks again after ten minutes. |
| GitHub did not answer in time, or did not answer the request | Check that machine's network, and that the login can read the repositories. |
| the team's node does not give delivery numbers | That node is older than this feature: update it. |
| the team's node does not give delivery numbers to this account | The account is an answerer in the org, who does not read them. |
| a repository of the team is not written owner/name | Fix `delivery.repos` in the configuration of the node the team runs on. |
| the team's node did not answer | That node is down or unreachable. |

## The org's numbers

The org states what it measures (its KPIs) in its definition, with no number. The numbers themselves (revenue, retention, a lead time) are business data, and the node keeps them in `outcomes/<org>.jsonl` of its data directory, one line for each number, with the directory readable by its owner only (`0700`, the file `0600`). Nothing else the node serves holds them, and only the node's owner reads or enters them: `agency outcome-add --org acme --kpi revenue --date 2026-11-24 --value 1350000` enters one, and `agency outcome-list --org acme --kpi revenue` lists a KPI's numbers.

- **Back this directory up, and keep it out of anything shared.** A backup of the data directory holds the numbers, so treat it like the account key.
- The numbers are for reading how a measure moves, not for accounting.
- **The readings.** `GET /api/orgs/<org>/outcomes/readings` (the owner only, like the numbers) says how each KPI a team's plan aims an epic at stood around the day the epic shipped. A team's view keeps a finished epic for a limited time (30 days for the scrum harness), so the node writes the day it first sees an aimed epic finished into `outcomes/<org>.ships.jsonl` (the same directory, the file `0600`; it holds a team, an epic and a time, and no number) and never changes it. Keep it with the numbers, and back it up the same way: without it, a reading of an epic that left the team's view cannot be made.

## Updating a node

```sh
cd ~/src/agency
git pull
(cd web && npm ci && npm run build)
cargo build --release -p agency-node
cp target/release/agency ~/.local/bin/agency.new && mv ~/.local/bin/agency.new ~/.local/bin/agency
launchctl kickstart -k gui/$(id -u)/local.agency.node
```

On Linux, restart the unit instead of the last line: `systemctl --user restart agency-node`.
With a release build, unpack the new tarball and install its binary as in "A release build", then restart the unit.
On macOS, `scripts/deploy-macos.sh` does these steps with a check at the end and a way back:

```sh
scripts/deploy-macos.sh --agency ~/.local/bin/agency --dir ~/.agency --label local.agency.node
```

- It builds the web client and the node from the repository the script is in. It refuses a working tree with uncommitted changes (`--allow-dirty` allows one), and checks that the new binary starts and serves on a throwaway data directory first (port 4790, or `SMOKE_PORT`; `--skip-preflight` leaves the check out). `--binary <file>` deploys a binary you built, without building.
- It puts the new binary in place with one rename, and keeps the one it replaces as `agency.previous` next to it. The running node keeps the file it started from until the restart.
- It restarts the job, then waits up to 60 seconds (`--timeout`) for the node to answer at its `listen` address (`--url` names another) from a new process, and for that process to keep running for 5 seconds (`--settle`), so a node that starts and stops again is not taken for a good one.
- If the node does not answer, it puts the previous binary back, restarts the job again, and checks again. The exit status says what happened: 0 deployed; 1 not deployed, and the previous binary is back and answering; 2 not deployed, and the previous binary does not answer either (look at the node now).
- `--rollback` swaps the installed binary and the previous one, restarts, and checks the same way; running it twice goes back and forth.

The job must already be loaded (see "Run it with launchd"). The script does not touch the data directory or the job's file. Only one deploy runs at a time.

The data directory is not touched.
The node picks a new write token each time it starts, so open pages reload once before their next write.

## Troubleshooting

### Log lines

The node writes these to `~/.agency/logs/serve.log`:

| Line | Meaning |
|---|---|
| `Node running: <url> (listening on <address>)` and `Web client: …` | The node started. |
| `agency: cannot listen on 127.0.0.1:4400: Address already in use` | Another process holds the port; launchd or systemd tries again every 10 seconds. `lsof -nP -iTCP:4400 -sTCP:LISTEN` shows which. |
| `agency: team acme/app runs on this node (…), but the org places it at …` | The node's `url` changed. Run the `agency team-move` the line gives. |
| `agency: the access setting is malformed (…)` | Fix `access` in `config.json`. Until then, every request through Cloudflare is refused. |
| `agency: the access appId is malformed (…)` | Fix `appId`. The rest of `access` still works; the app sign-in does not. |
| `agency: refused an Access request (no Access token)` | A request came through Cloudflare without an Access sign-in: a bypass covers more than the two paths, or Access is not in front of the host name. |
| `agency: refused an Access request (wrong audience)` | `audience` is not the application's AUD tag. |
| `agency: refused an Access request (wrong issuer)` | `teamDomain` is not your team domain. |
| `agency: refused an Access request (expired)` or `(not valid yet)` | The sign-in expired, or this machine's clock is off. |
| `agency: refused an Access request (cannot fetch the Access certs: …)`, `(unknown signing key)`, or `(the signature does not match)` | The node cannot get Access's keys from `teamDomain`, or the token was not made for this team domain. |
| `agency: team acme/app: the team runtime fails: …` | The team's runtime could not run or answered 5xx; the rest of the line says why (see the answers below). It is written once when the runtime starts failing and again only if the reason changes. |
| `agency: team acme/app: the team runtime answers again` | The runtime works again after a failure. |
| `agency: refused GET /fed/v1/…: …` | Another node's request was refused, and the line says why (for example "a read signed by bob at …, 400 seconds behind this node's clock; check the clocks"). The caller only got "not found". At most 20 such lines are written a minute, then one line says how many more were not; a refusal of a request validly signed by a member is always written. |
| `agency: push notifications are off: …` | The `push` key cannot be read. Nothing is sent. |
| `agency: a push notification was not sent: …` | APNs refused one; it is tried again 30 seconds later. |
| `agency: usage of "<id>" could not be read: …; trying again in <n> minutes` | The collector of that funding source failed; the text says why (no login found, expired token, refused token, the service unavailable). The last usage file stays. Log in again in that source's configuration directory, or use Claude Code with it, and the next read succeeds. `agency: usage of "<id>" is read again` follows. |
| `agency: re-reading the replicas failed: …`, `agency: checking for new requests failed: …`, or `agency: retrying the outbox failed: …` | The node could not write to its data directory (for example, the disk is full). Another node that cannot be reached is not logged: `agency status` lists what waits in the outbox, and the web client shows when the replica of a team on another node was last read. |
| `agency: team acme/app: a command the team runtime runs was not found: …` | The team runtime failed because a command it runs (such as `gh` or `node`) is not on the `PATH` of the launchd agent or the systemd unit. Check with `command -v` in your terminal, run the script of "Run it with launchd" or "Run it with systemd" again from a terminal where they work, and restart. It follows the line `the team runtime fails: … ENOENT`. |
| `agency: <message>`, and the node stops | It could not start, for example because `config.json` does not parse. launchd or systemd tries again every 10 seconds. |

### Answers from the node

The node's answers, in the browser or with `curl`, say what went wrong in their `error`:

| Answer | Meaning |
|---|---|
| `503` `cannot start the team runtime: No such file or directory` | The team's runtime command is not at the path in `config.json` (`teams[].runtime.command`). |
| `503` `the team runtime did not answer within 20 seconds` | The runtime hangs, or is too slow. |
| `503` `the team runtime did not write an agency-runtime/1 response (…): …` | The runtime failed; the end of the answer is what it wrote to stderr. Often a tool missing from the `PATH` of the launchd agent or the systemd unit. |
| `421` `wrong host` | The page was opened at an address other than `127.0.0.1:<port>` or `localhost:<port>`. |
| `403` `the Client API is local only` | A request from another machine reached the node without Cloudflare. |
| `403` `missing or wrong write token` | The page is older than the node's last start. Reload it. |
| `403` `only the node's owner may decide` | Signed in to Access with an email other than `ownerEmail`. |

## The team runtime

The node never reads the harness's files. For each team, it runs the team's runtime command (`<scrum> runtime`, with `SCRUM_STATE_DIR` set to the team's state directory) and talks to it in agency-runtime/1; see [protocol.md](protocol.md#agency-runtime1).
Set up the harness by its own instructions. What the node needs from it:

- A command that answers agency-runtime/1: `describe`, `view`, and `decide`, and `tokenUsage` for the costs screen. Each call must answer within 20 seconds.
- The team's state directory, the same one as `team-add --state-dir`.
- Every tool the runtime calls (for the scrum harness `node` and `gh`), on the `PATH` written into the launchd agent or the systemd unit. A command that is not there stops the runtime, and the node's log says so.
- Requests for you leave `to` out or set it to `owner`. In an org where you are the only member, every request is yours. A participant waiting on you has `waitingOn` set to `{ "kind": "person", "person": "owner" }`.

While it runs, the node asks the runtime for the team's view every 30 seconds and each time a page reads the team, so `view` should be cheap.
