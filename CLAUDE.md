# CLAUDE.md

Conventions for everyone (people and agents) working in this repository.

## Language

English is the official language of this repository: code, comments, doc comments, test names, CLI output, error messages, docs, commit messages, issues, and pull requests.

## Public repository hygiene

This repository is public. Never write names or details that point at a specific company, customer, person, or internal event: no real organization or product names, customer identifiers, people's names, real issue numbers from other projects, internal tool names, credentials, account IDs, or local paths. Use fictional examples (`acme`, `app`, `alice`, `bob`). Read your diff for this before every push.

## Architecture

- `agency-proto` is the wire protocol: canonical JSON, Ed25519 signatures, envelopes, org definitions, invites. `docs/protocol.md` is the specification and `testdata/vectors.json` pins its byte-level behavior. A protocol change updates the spec, the types, and the test vectors in the same pull request.
- `agency-core` holds the domain, use cases, and ports (traits). It must not depend on infrastructure crates (tokio, axum, reqwest, filesystem). Time, randomness, storage, keys, peers, and team runtimes go through ports.
- `agency-adapters` implements the ports. `agency-node` wires adapters together and provides the CLI.
- Clients (web, iOS, desktop) talk to their home node's Client API. A person's private key stays on their client; nodes never sign decisions on a person's behalf.
- Types shared with TypeScript are generated from the Rust types, never copied by hand.

## Workflow

- Branch from `origin/main` (`feat/...`, `fix/...`, `refactor/...`, `docs/...`, `chore/...`) and open a pull request to `main`.
- Commit messages and pull request titles follow Conventional Commits and say why the change is needed.
- Pull request body: **Why**, **What**, and **How** only when the implementation needs explaining. End with `Closes #N` on its own line when the pull request completes an issue.
- Attach screenshots to pull requests that change what the user sees.
- Before opening a pull request: `cargo test`, `cargo clippy --all-targets`, and, for the web client, `npm test` and `npm run build`.
- Security-critical code (signature checks, membership, expiry, replay protection, store-and-forward) needs tests that fail when the deciding line is removed. Check this by mutating the line before you ask for review.
