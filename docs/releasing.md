# Releasing

The node is released as versions of the whole repository: the node, the protocol it speaks, and the web client it embeds go out together under one number. Releases are made from the Conventional Commits titles of the merged pull requests, by [release-please](https://github.com/googleapis/release-please). Nobody edits a version or `CHANGELOG.md` by hand.

## The version number

Versions are `0.MINOR.PATCH` until the first 1.0. How the number moves, from the titles since the last release:

| Titles since the last release | Next version |
|---|---|
| a `feat`, or a breaking change (`feat!:`, `fix!:`, or a `BREAKING CHANGE:` footer) | the minor number goes up (0.2.0 to 0.3.0) |
| only `fix` and `perf` | the patch number goes up (0.2.0 to 0.2.1) |
| only `docs`, `test`, `chore`, `ci`, `build`, `style`, `refactor` | no release is proposed |

A `refactor` is listed in the changelog, but it does not call for a release by itself. Before 1.0, a breaking change raises the minor number, as a new feature does, so read the changelog before updating across a minor number. What counts as breaking at each boundary (nodes, the Client API, the team runtime, the apps) is the compatibility policy's subject, and the release numbering follows it.

## How a release is made

1. Merge pull requests as usual. The pull request title is the commit title (a squash merge), so it must follow Conventional Commits.
2. release-please keeps one open pull request for the node, **chore: release X.Y.Z**, that raises the version in `Cargo.toml`, and in the web client's `package.json` and `package-lock.json`, and adds the new section of `CHANGELOG.md`. A second job of the same workflow adds a commit that brings the workspace crates' versions in `Cargo.lock` up to date (release-please cannot do that itself). The pull request updates itself after each merge to `main`.
3. To release, merge that pull request. release-please then tags `vX.Y.Z` and makes the GitHub Release with the changelog section as its notes.
4. The `release` workflow builds the node for Linux (x86_64 and arm64) from the tag and attaches each tarball and its `.sha256` to the release.
5. The macOS builds are made on a Mac, never on a GitHub runner. After step 4, on a Mac with the repository, Rust, and Node installed:

```sh
git fetch --tags
git checkout vX.Y.Z
scripts/release-macos.sh vX.Y.Z
```

The script refuses a tree that is not at the tag, builds the web client and the node, checks that the binary starts and serves, and attaches `agency-X.Y.Z-macos-<arm64|x86_64>.tar.gz` and its `.sha256` to the release. Run it once on each kind of Mac that should have a build.

The macOS binaries are not signed and not notarized. A browser download is quarantined by macOS and will not open until `xattr -d com.apple.quarantine agency` (`gh release download` and `curl` do not quarantine).

## Tags: one artifact, one prefix

The repository makes more than one artifact. The node is its main one, and its tags are plain: `vX.Y.Z`. A release is never tagged again once it exists. Every other artifact has a component prefix, so that tags of different artifacts never look alike:

| Artifact | Tag | Where its version is kept | Its changelog |
|---|---|---|---|
| The node, with the protocol and the web client | `vX.Y.Z` | `Cargo.toml`, the web client's `package.json` and `package-lock.json` | `CHANGELOG.md` |
| The iOS app | `ios-vX.Y.Z` | `MARKETING_VERSION` in `apple/project.yml` | `apple/CHANGELOG.md` |
| An Android app, a macOS app (later) | `android-vX.Y.Z`, `macos-vX.Y.Z` | | |

The `scrum-harness` repository has one artifact, so its tags are plain `vX.Y.Z`.

Each artifact is versioned from the commits that touch its own files. A commit that touches the node and the iOS app (a change to a type that both use, for example) is in both changelogs. The `apple/` folder is left out of the node's releases, so a change to the app alone does not call for a node release.

## The iOS app

release-please keeps a second pull request open, **chore: release ios X.Y.Z**, that raises `MARKETING_VERSION` in `apple/project.yml` (the version the App Store shows) and adds the section of `apple/CHANGELOG.md`. The numbers move as for the node, from the titles of the commits that touch `apple/`. Merging it tags `ios-vX.Y.Z` and makes a GitHub Release with the changelog as its notes. The app is delivered through TestFlight, so that release has no files.

The build number stays the UTC time of the upload (`apple/scripts/testflight.sh`), so it rises with each upload of any version. To send a release to TestFlight, check out its tag and run the script:

```sh
git fetch --tags
git checkout ios-vX.Y.Z
apple/scripts/testflight.sh
```

Each build says which source it was made from, in its `Info.plist`: `AgencyBuildCommit` (the commit) and `AgencyBuildDescribe` (`git describe` against the `ios-v` tags, such as `ios-v0.2.0-0-g1a2b3c4d5e6f` for the tag itself, or `ios-v0.2.0-3-g…` for a build three commits after it; `-dirty` when the working tree had uncommitted changes). The script also appends a line to `apple/uploads.log` (not committed) for every upload that succeeded: the time, the build number, the version, the commit, and the describe. A build run from Xcode says `unknown`.

## What a release holds

For each of macOS (arm64, x86_64) and Linux (x86_64, arm64): `agency-<version>-<os>-<arch>.tar.gz` and `agency-<version>-<os>-<arch>.tar.gz.sha256`. The tarball holds the `agency` binary (with the web client inside), the script that runs it as a service (`systemd-unit.sh` or `launchd-plist.sh` and `deploy-macos.sh`), the install guides, and the licenses. [install.md](install.md) says how to install it.

Builds that are not of a release (a manual run of the `release` workflow, or a pull request that changes it) are workflow artifacts, named with the short commit.

## Settings it needs

- Settings, Actions, General: **Allow GitHub Actions to create and approve pull requests** is on (release-please opens its pull request with the workflow's own token). The default token permissions stay read-only; the workflows ask for what they need.
- A pull request made with the workflow's own token does not start other workflows, so the release pull request has no CI run. It changes only versions and the changelog.
- The first release, 0.1.0, counted the pull requests merged from the day releases were set up; the history before it is in `git log`.
