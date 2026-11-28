# CLAUDE.md

Conventions for everyone (people and agents) working in this repository.

## Language

English is the official language of this repository: code, comments, doc comments, test names, CLI output, error messages, docs, commit messages, issues, and pull requests.

Words follow `docs/glossary.md`, the one place the project's terms are defined: code, comments, docs, issues, pull requests, and the screens' text in every language use its terms and avoid its "Do not use" words. When a change needs a new term or a different meaning, change the glossary first, in the same pull request or before it.

User-facing installation docs come in English and Japanese (`docs/install.md` is the source, `docs/install.ja.md` follows it section by section). A change to one changes the other in the same pull request. Japanese text uses the glossary's Japanese names.

`docs/data.md` says what a node keeps, where, for how long, and how to delete it, its keys, and what leaves the machine. It follows `docs/protocol.md`: a change to what a node stores, keeps, or sends changes it in the same pull request (a test checks that every file the store writes is in it).

## Public repository hygiene

This repository is public. Never write names or details that point at a specific company, customer, person, or internal event: no real organization or product names, customer identifiers, people's names, real issue numbers from other projects, internal tool names, credentials, account IDs, or local paths. Use fictional examples (`acme`, `app`, `alice`, `bob`). Read your diff for this before every push.

## Architecture

- `agency-proto` is the wire protocol: canonical JSON, signatures (Ed25519, and P-256 for device keys), envelopes, org definitions, invites. `docs/protocol.md` is the specification and `testdata/vectors.json` pins its byte-level behavior. A protocol change updates the spec, the types, and the test vectors in the same pull request.
- `agency-core` holds the domain, use cases, and ports (traits). It must not depend on infrastructure crates (tokio, axum, reqwest, filesystem). Time, randomness, storage, keys, peers, and team runtimes go through ports.
- `agency-adapters` implements the ports. `agency-node` wires adapters together and provides the CLI.
- Clients (web, iOS, desktop) talk to their home node's Client API. Clients sign decisions with device keys they hold (non-extractable where the platform allows); nodes never sign envelopes, and team nodes accept only device-signed ones. The account key stays on the node for signed reads, join requests, and adding a person's first device.
- Types shared with TypeScript are generated from the Rust types, never copied by hand.
- Config and definition structs (HostedTeam, TeamPlace, ...) are built through their constructor and its `with_` methods, never as struct literals outside their crate; a test in `agency-core` (`tests/built_through_constructors.rs`) fails if a source does.
- Clients never show a placeholder value for something the node has not answered yet: no `?? 0`, `|| 0`, `?? []` or default state standing in for a node value in a view. Until the answer comes, the region shows a loading state (a skeleton under Suspense on the web, a redacted or progress state in the Apple apps); a failed read shows its failure. A number on screen is always one the node gave, so 0 means 0.

## Workflow

- Branch from `origin/main` (`feat/...`, `fix/...`, `refactor/...`, `docs/...`, `chore/...`) and open a pull request to `main`.
- Commit messages and pull request titles follow Conventional Commits and say why the change is needed.
- Pull request body: **Why**, **What**, and **How** only when the implementation needs explaining. End with `Closes #N` on its own line when the pull request completes an issue.
- Attach screenshots to pull requests that change what the user sees.
- Before opening a pull request: `cargo test`, `cargo clippy --all-targets`, and, for the web client, `npm test` and `npm run build`.
- Merging goes through GitHub's merge queue: nobody merges a pull request by hand. When a pull request is approved and its checks are green, the lead adds it to the queue (the queue button, or `gh pr merge --auto`), which tests it together with the ones ahead of it on top of `main` and merges it when that passes; one that fails is taken out of the queue with its failing check named. Only `ci.yml` listens to `merge_group`, so that the required checks report for a queued group: a workflow that writes (a release, a tag, a comment, a branch) must not listen to it, since a group has not merged yet and may be thrown away.
- Security-critical code (signature checks, membership, expiry, replay protection, store-and-forward) needs tests that fail when the deciding line is removed. Check this by mutating the line before you ask for review.
