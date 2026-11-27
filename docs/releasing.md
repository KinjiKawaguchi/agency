# Releasing

The node is released as versions of the whole repository: the node, the protocol it speaks, and the web client it embeds go out together under one number. Releases are made from the Conventional Commits titles of the merged pull requests, by [release-please](https://github.com/googleapis/release-please). Nobody edits a version or `CHANGELOG.md` by hand.

## The version number

Versions are `0.MINOR.PATCH` until the first 1.0. How the number moves, from the titles since the last release:

| Titles since the last release | Next version |
|---|---|
| a `feat`, or a breaking change (`feat!:`, `fix!:`, or a `BREAKING CHANGE:` footer) | the minor number goes up (0.2.0 to 0.3.0) |
| only `fix` and `perf` | the patch number goes up (0.2.0 to 0.2.1) |
| only `docs`, `test`, `chore`, `ci`, `build`, `style`, `refactor` | no release is proposed |

A `refactor` is listed in the changelog, but it does not call for a release by itself. Before 1.0, a breaking change raises the minor number, as a new feature does, so read the changelog before updating across a minor number. What counts as breaking at each boundary (nodes, the Client API, the team runtime, the apps) is the subject of the compatibility policy ([compatibility.md](compatibility.md)), and the release numbering follows it.

## How a release is made

1. Merge pull requests as usual. The pull request title is the commit title (a squash merge), so it must follow Conventional Commits.
2. release-please keeps one open pull request for the node, **chore: release X.Y.Z**, that raises the version in `Cargo.toml`, and in the web client's `package.json` and `package-lock.json`, and adds the new section of `CHANGELOG.md`. A second job of the same workflow adds a commit that brings the workspace crates' versions in `Cargo.lock` up to date (release-please cannot do that itself). The pull request updates itself after each merge to `main`.
3. To release, merge that pull request. release-please then tags `vX.Y.Z` and makes the GitHub Release with the changelog section as its notes.
4. The `release` workflow builds the node for Linux (x86_64 and arm64) from the tag and attaches each tarball and Debian package, with their `.sha256` files, to the release, with the SBOMs and the signed `SHA256SUMS` (see "What a release holds").
5. The macOS builds are made on a Mac, never on a GitHub runner. After step 4, on a Mac with the repository, Rust, and Node installed:

```sh
git fetch --tags
git checkout vX.Y.Z
scripts/release-macos.sh vX.Y.Z
```

The script refuses a tree that is not at the tag, builds the web client and the node, checks that the binary starts and serves, and attaches `agency-X.Y.Z-macos-<arm64|x86_64>.tar.gz` and its `.sha256` to the release. Run it once on each kind of Mac that should have a build.

6. Check the release as someone who downloads it would, with the commands of "Checking a release" below: the signature of `SHA256SUMS` and the sums of the files. The signature is made by the workflow's identity, `release.yml` at `refs/heads/main` (release-please runs on `main` and calls it), and the expression that names it ends there so that a signature made from any other ref never verifies. The first release is the first time this is run for real: if `cosign verify-blob` refuses the real signature, read the identity in the certificate (`cosign verify-blob` says what it found), correct the expression here, and say so in the release notes.

The macOS binaries are not signed and not notarized. A browser download is quarantined by macOS and will not open until `xattr -d com.apple.quarantine agency` (`gh release download` and `curl` do not quarantine).

## Tags: one artifact, one prefix

The repository makes more than one artifact. The node is its main one, and its tags are plain: `vX.Y.Z`. A release is never tagged again once it exists. Every other artifact has a component prefix, so that tags of different artifacts never look alike:

| Artifact | Tag | Where its version is kept | Its changelog |
|---|---|---|---|
| The node, with the protocol and the web client | `vX.Y.Z` | `Cargo.toml`, the web client's `package.json` and `package-lock.json` | `CHANGELOG.md` |
| The Apple apps: iPhone and Mac, one version | `apple-vX.Y.Z` | `MARKETING_VERSION` in `apple/project.yml` | `apple/CHANGELOG.md` |
| An Android app (later) | `android-vX.Y.Z` | | |

The `scrum-harness` repository has one artifact, so its tags are plain `vX.Y.Z`.

Each artifact is versioned from the commits that touch its own files. A commit that touches the node and the Apple apps (a change to a type that both use, for example) is in both changelogs. The `apple/` folder is left out of the node's releases, so a change to the apps alone does not call for a node release.

## The Apple apps

The iPhone app and the Mac app have the same name and bundle ID (one App Store Connect record) and one version: `MARKETING_VERSION` is shared by the iPhone app, the Mac app, and the widgets in `apple/project.yml`. A fix for one platform alone is the same version with a higher build number of that platform, not a version of its own.

release-please keeps a second pull request open, **chore: release apple X.Y.Z**, that raises `MARKETING_VERSION` in `apple/project.yml` (the version the App Store shows) and adds the section of `apple/CHANGELOG.md`. The numbers move as for the node, from the titles of the commits that touch `apple/`. Merging it tags `apple-vX.Y.Z` and makes a GitHub Release with the changelog as its notes. The apps are delivered through TestFlight, so that release has no files.

The build number stays the UTC time of the upload (`apple/scripts/testflight.sh`), so it rises with each upload of any version. To send a release to TestFlight, check out its tag and run the script:

```sh
git fetch --tags
git checkout apple-vX.Y.Z
apple/scripts/testflight.sh
```

Each build says which source it was made from, in its `Info.plist`: `AgencyBuildCommit` (the commit) and `AgencyBuildDescribe` (`git describe` against the `apple-v` tags, such as `apple-v0.2.0-0-g1a2b3c4d5e6f` for the tag itself, or `apple-v0.2.0-3-g…` for a build three commits after it; `-dirty` when the working tree had uncommitted changes). The script also appends a line to `apple/uploads.log` (not committed) for every upload that succeeded: the time, the build number, the platform (`ios` or `macos`: the two apps have the same version, so this tells their builds apart), the version, the commit, and the describe. A build run from Xcode says `unknown`.

## What a release holds

For each of macOS (arm64, x86_64) and Linux (x86_64, arm64): `agency-<version>-<os>-<arch>.tar.gz` and `agency-<version>-<os>-<arch>.tar.gz.sha256`. Each Linux build also has `agency_<version>_<amd64|arm64>.deb` and its `.sha256`. The tarball holds the `agency` binary (with the web client inside), the script that runs it as a service (`systemd-unit.sh` or `launchd-plist.sh` and `deploy-macos.sh`), the install guides, and the licenses. The Debian package installs the binary, the guides, licenses, and its user-unit script, but not a system service. [install.md](install.md) says how to install it.

The release workflow also writes the lists of what the node is made of, as CycloneDX (JSON), one for the node's Rust crates (`agency-<version>-sbom-rust.cdx.json`, from `Cargo.lock`, for every platform the node is built for, so it may name a crate that one build does not contain) and one for the npm packages the web client embeds (`agency-<version>-sbom-web.cdx.json`, from `web/package-lock.json`, without the tools that only test and build). Last, it writes `SHA256SUMS`, the SHA-256 of every file above (the builds, the Debian packages and the lists, not the per-file `.sha256` files), and signs it: `SHA256SUMS.sigstore.json` is the signature, made by Sigstore's keyless signing with the workflow's own identity, so no signing key is kept anywhere. The macOS builds are made on a Mac after this and are in neither the list nor the signature.

Builds that are not of a release (a manual run of the `release` workflow, or a pull request that changes it) are workflow artifacts, named with the short commit.

## Checking a release

To check that a download is what the release workflow built, with [cosign](https://docs.sigstore.dev/cosign/) and `sha256sum` (`<owner>/<repo>` is this repository, and `<tag>` the release):

```sh
gh release download <tag> --repo <owner>/<repo> --pattern 'SHA256SUMS*' --pattern 'agency-*-linux-x86_64.tar.gz'
cosign verify-blob SHA256SUMS \
  --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp '^https://github\.com/<owner>/<repo>/\.github/workflows/release\.yml@refs/heads/main$' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
sha256sum --check --ignore-missing SHA256SUMS
```

The first command takes the signed list and the files you want, the second checks that the list was signed by this repository's release workflow, and the third that your files are the ones in it. A file that is not in the list, such as a macOS build, has only its own `.sha256`, which tells you the download is whole but not who made it.

The lists of materials are ordinary CycloneDX files, to be read by any tool that reads them; they are in the same list of checksums. Each is the list of what went into the build, as it is in `Cargo.lock` and `web/package-lock.json` at the tag, so a newly reported vulnerability in a dependency can be matched against a release you run.

CI fails a change that takes the lists, the checksums, or the signature out of the release workflow (`scripts/ci/check-release-workflow.sh`). The release workflow itself runs on a pull request that changes it, up to the signing, which needs a tag.

## Settings it needs

- Settings, Actions, General: **Allow GitHub Actions to create and approve pull requests** is on (release-please opens its pull request with the workflow's own token). The default token permissions stay read-only; the workflows ask for what they need.
- A pull request made with the workflow's own token does not start other workflows, so the release pull request has no CI run. It changes only versions and the changelog.
- The first release, 0.1.0, counted the pull requests merged from the day releases were set up; the history before it is in `git log`.
