# Security policy

## Reporting a vulnerability

Report a vulnerability **privately**, with GitHub's private vulnerability reporting: on this repository's page, open the Security tab and choose "Report a vulnerability". Please do not open a public issue or pull request for it, and do not post it anywhere else before a fix is released.

A report is most useful with:

- what you found and where (the file, route or screen);
- the version or commit you tested, and how the node was set up (behind an identity provider or not, local or reachable from outside);
- the steps to reproduce it, or a proof of concept that does no harm;
- what an attacker gains: whose decision, whose data, or which key.

You get a first answer within 5 working days. It says whether we take the report as a vulnerability, and who looks at it. We then tell you when it is fixed, and a fix is a release that the changelog marks as a security fix, with credit to you unless you prefer not to be named. We ask you to keep the report private until that release is out, and we do not ask you to wait longer than needed.

## What is in scope

- The node (`agency`): the federation routes (`/fed/v1`), the Client API (`/api`), who is let in, and what the node keeps.
- The protocol in [docs/protocol.md](docs/protocol.md): the signed texts, the org definition, devices, invites and claim links.
- The web client the node serves, and the Apple apps.
- What a release is made of and how it is built: the release workflow, the checksums and the lists of materials ([docs/releasing.md](docs/releasing.md), "Checking a release").

[docs/security.md](docs/security.md) says who is trusted for what and what is known not to be covered. A report that shows one of its claims to be wrong is the most valuable kind.

## What is not in scope

- Weaknesses of a service Agency uses (the identity provider, GitHub, a push service), which are for that service. How Agency uses it is in scope.
- Anything that needs an attacker who already controls the machine a node runs on, as the node's operating system user. The limits in [docs/security.md](docs/security.md), "Known limits", are known.
- A node set up against its documentation, for example reachable from outside with no identity provider in front of its Client API.
- Floods of requests that only a network or a provider can absorb.
- Findings of an automatic scanner that name no way to use them.
- The team runtimes (harnesses). Each has its own repository and its own policy.

## Supported versions

Before version 1.0, a fix is made in the latest release only; update to it. The version a fix is in is in the changelog.

## Checking what you install

Every release carries lists of what the node is made of (SBOMs) and a signed list of checksums. [docs/releasing.md](docs/releasing.md) says how to check them.
