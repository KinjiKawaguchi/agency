# CLAUDE.md

Conventions for everyone (people and agents) working in this repository.

## Language

English is the official language of this repository: code, comments, doc comments, test names, CLI output, error messages, docs, commit messages, issues, and pull requests.

Words follow `docs/glossary.md`, the one place the project's terms are defined: code, comments, docs, issues, pull requests, and the screens' text in every language use its terms and avoid its "Do not use" words. When a change needs a new term or a different meaning, change the glossary first, in the same pull request or before it.

User-facing installation docs come in English and Japanese (`docs/install.md` is the source, `docs/install.ja.md` follows it section by section). A change to one changes the other in the same pull request. Japanese text uses the glossary's Japanese names.

## Public repository hygiene

This repository is public. Never write names or details that point at a specific company, customer, person, or internal event: no real organization or product names, customer identifiers, people's names, real issue numbers from other projects, internal tool names, credentials, account IDs, or local paths. Use fictional examples (`acme`, `app`, `alice`, `bob`). Read your diff for this before every push.

## Architecture

- `agency-proto` is the wire protocol: canonical JSON, signatures (Ed25519, and P-256 for device keys), envelopes, org definitions, invites. `docs/protocol.md` is the specification and `testdata/vectors.json` pins its byte-level behavior. A protocol change updates the spec, the types, and the test vectors in the same pull request.
- `agency-core` holds the domain, use cases, and ports (traits). It must not depend on infrastructure crates (tokio, axum, reqwest, filesystem). Time, randomness, storage, keys, peers, and team runtimes go through ports.
- `agency-adapters` implements the ports. `agency-node` wires adapters together and provides the CLI.
- Clients (web, iOS, desktop) talk to their home node's Client API. Clients sign decisions with device keys they hold (non-extractable where the platform allows); nodes never sign envelopes, and team nodes accept only device-signed ones. The account key stays on the node for signed reads, join requests, and adding a person's first device.
- Types shared with TypeScript are generated from the Rust types, never copied by hand.
- Config and definition structs (HostedTeam, TeamPlace, ...) are built through their constructor and its `with_` methods, never as struct literals outside their crate; a test in `agency-core` (`tests/built_through_constructors.rs`) fails if a source does.

## Workflow

- Branch from `origin/main` (`feat/...`, `fix/...`, `refactor/...`, `docs/...`, `chore/...`) and open a pull request to `main`.
- Commit messages and pull request titles follow Conventional Commits and say why the change is needed.
- Pull request body: **Why**, **What**, and **How** only when the implementation needs explaining. End with `Closes #N` on its own line when the pull request completes an issue.
- Attach screenshots to pull requests that change what the user sees.
- Before opening a pull request: `cargo test`, `cargo clippy --all-targets`, and, for the web client, `npm test` and `npm run build`.
- Security-critical code (signature checks, membership, expiry, replay protection, store-and-forward) needs tests that fail when the deciding line is removed. Check this by mutating the line before you ask for review.
