# Agency for iPhone

`apple/` holds a thin iPhone app. It opens the owner's home node in a web view, so everything the web client does works in it. Push notifications are built on top of it.

```
apple/
  AgencyKit/            Swift package: home node address validation and the navigation allow list.
                        No UIKit, so it builds and tests on macOS and Linux.
  App/                  SwiftUI app: first-launch setup, the web view, the failure page.
  Config/Shared.xcconfig  Defaults; includes Local.xcconfig when it exists.
  Local.xcconfig.example  Template for the signing values. Copy it to Local.xcconfig (not committed).
  ExportOptions.plist   Export settings for App Store Connect.
  project.yml           XcodeGen project definition.
  scripts/make-icon.swift  Draws the app icon.
  scripts/register-bundle-id.sh  Registers the bundle ID and enables Push Notifications.
  scripts/testflight.sh  Archives and uploads one build to TestFlight.
  scripts/test/run.sh   Offline tests of the two scripts.
```

## How the app behaves

- On the first launch it asks for the home node address and keeps it in `UserDefaults`. Only the origin is kept: a pasted path is dropped. Plain `http` is accepted only for `localhost`, `127.0.0.1`, and `.local` names, so a node on the owner's machine can be tried.
- It loads the node in a `WKWebView` that uses the default website data store, which is persistent. The Cloudflare Access session cookie and the web client's own storage survive app launches. How long the sign-in lasts is the session duration of the Access application.
- Only the home node and `<team>.cloudflareaccess.com` (https) load inside the app. Any other `http(s)` address opens in Safari, `mailto:` and `tel:` go to the system, and every other scheme is ignored. A link with `target="_blank"` follows the same rule instead of opening a window. The rule is `NavigationPolicy` in `apple/AgencyKit`.
- Sign-in through an identity provider other than the Access one-time PIN needs the provider's host on that allow list, or its pages open in Safari and the session never reaches the app. When the app is built for the node's host (see "Signing in with a passkey"), it does not show the Access login page in the web view at all: it signs in in the system's web authentication session instead, where passkeys work.
- Pull down to reload. When a page cannot be loaded, or the node answers its own page with HTTP 400 or higher, a native page offers Reload and Change home node.
- The user agent carries `AgencyApp`, so the web client can tell it runs in the app.

## Build

You need Xcode and [XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`).

```sh
cd apple
xcodegen generate
xcodebuild -project Agency.xcodeproj -scheme Agency \
  -destination 'generic/platform=iOS' -configuration Release \
  CODE_SIGNING_ALLOWED=NO build
```

The generated `Agency.xcodeproj` and `App/Info.plist` are not committed. They derive entirely from `project.yml`, a regenerated project file produces merge conflicts, and the signing values must not leak into the repository.

Xcode puts build output in `~/Library/Developer/Xcode/DerivedData`. Delete the folder of this project when you are done, or pass `-derivedDataPath` to keep it somewhere you can remove.

Test the package, with no simulator and no Xcode project:

```sh
cd apple/AgencyKit && swift test
```

Continuous integration runs the same command on Linux. The app itself is not built in CI, because the repository has no macOS runner.

## Signing values

Copy `apple/Local.xcconfig.example` to `apple/Local.xcconfig` and set:

- `DEVELOPMENT_TEAM`: the Apple Developer team ID.
- `PRODUCT_BUNDLE_IDENTIFIER`: the bundle ID of the app record in App Store Connect.
- `AGENCY_ASSOCIATED_HOST`: the host name of your home node. It goes into the app's associated domains and tells the app which node may use the passkey sign-in. Optional: without it the app shows the Access login page in its web view.

`Local.xcconfig` is ignored by git. Without it the project builds with a placeholder bundle ID, which cannot be signed.

## Release in two commands

Two scripts in `apple/scripts` take the app from a bundle ID to a TestFlight build. Both read the App Store Connect API key from the environment and from a file outside the repository, and print none of it.

Once, by hand:

1. Create an App Store Connect API key (Users and Access, Integrations) with the App Manager role. Keep the `.p8` file outside the repository (the scripts refuse a key file inside it). Note the key ID and the issuer ID.
2. Fill in `apple/Local.xcconfig` (see "Signing values").
3. Create the app record in App Store Connect with that bundle ID. The API cannot create app records, so this one step is in the web page.

Then set the key where the scripts can see it. Do not put these in a file in the repository:

```sh
export ASC_KEY_PATH=~/path/to/AuthKey_XXXXXXXXXX.p8   # the API key file
export ASC_KEY_ID=XXXXXXXXXX                           # its 10 character key ID
export ASC_ISSUER_ID=00000000-0000-0000-0000-000000000000
```

**`apple/scripts/register-bundle-id.sh`** registers the bundle ID from `Local.xcconfig` for iOS if the account does not have it, and enables the Push Notifications and Associated Domains capabilities on it. It signs the API token itself (ES256, valid for ten minutes) with `openssl`, and calls the API with `curl` and `jq`. It creates only what is missing, so it can be run again at any time, and it says what it did:

```
Registered bundle ID com.acme.agency.
Enabled Push Notifications.
Enabled Associated Domains.
```

The name of a new bundle ID is `Agency`; set `ASC_APP_NAME` to use another.

**`apple/scripts/testflight.sh`** does everything for one build:

1. generates the Xcode project (`xcodegen generate`),
2. archives the Release configuration with a build number made from the current UTC time (`BUILD_NUMBER` overrides it; the number must be higher than the last one uploaded for this version),
3. exports with `ExportOptions.plist`, whose `destination` is `upload` and `method` is `app-store-connect`, so the export step sends the build to App Store Connect,
4. deletes `apple/build/` and this project's DerivedData folder, also when a step failed.

Both archive and export run with `-allowProvisioningUpdates -authenticationKeyPath ... -authenticationKeyID ... -authenticationKeyIssuerID ...`, which lets Xcode create the certificate and provisioning profile itself.

Both scripts stop before doing anything, and list every problem at once, when `apple/Local.xcconfig` is missing or still holds the placeholders, a key setting is missing or malformed, or the key file is unreadable or inside the repository.

After processing (a few minutes), the build appears under TestFlight in App Store Connect. Add the owner as an internal tester. Internal testing needs no beta review.

To keep an `.ipa` instead of uploading, set `destination` to `export` in `ExportOptions.plist`; the script deletes `build/` at the end, so run the two `xcodebuild` steps from the script by hand and upload the `.ipa` with the Transporter app.

`MARKETING_VERSION` (the version the owner sees) is set in `project.yml`.

### Tests for the scripts

```sh
apple/scripts/test/run.sh
```

It needs `bash`, `openssl`, `perl` and `jq`, and uses no Apple account, network, or Xcode: a generated key signs a token and the test verifies the signature with the public key, the argument checks are exercised, and stand-ins replace `curl`, `xcodegen` and `xcodebuild`. Continuous integration runs it on Linux. A run against Apple itself has only been done by hand.

## Signing in with a passkey

A `WKWebView` cannot run WebAuthn for the Access team domain (`<team>.cloudflareaccess.com`): iOS allows it only for a relying party that is one of the app's associated domains, and that file would have to be served by Cloudflare. So an Access application that offers passkeys fails in the app's web view with "The passkey prompt was canceled." The one-time PIN still works there.

The app therefore signs in outside the web view, in an `ASWebAuthenticationSession`. It runs in Safari's context, so passkeys and password managers work, and it needs no password prompt of its own.

1. The web view is about to load an Access login page (`/cdn-cgi/access/login/...`). The app cancels that navigation and starts the session on `https://<node host>/app/handoff?state=<state>`. The `state` is 256 random bits made for this sign-in.
2. Access signs the owner in and returns to the handoff route, which sits behind Access. The node answers with a redirect to `https://<node host>/app/callback#token=<Access token>&state=<state>`.
3. That address is a universal link, so iOS hands it only to the app that the node's `/.well-known/apple-app-site-association` file names. The app checks the host, the path, that `state` is its own, and that the token is a JWT that has not expired (`AccessHandoff` in `apple/AgencyKit`), then writes the token to the web view as the `CF_Authorization` cookie: host-only, path `/`, `Secure`, `HttpOnly`, expiring with the token, as Access sets it. The web view loads the page it was on.

The token lives in the URL fragment so that it never reaches a server or a log. A callback that does not belong to a running session is ignored.

Whether it is used is decided by the build and by the node:

- The app uses it only when the home node is on `https` port 443 and its host is `AGENCY_ASSOCIATED_HOST`, because that is the only host a universal link can return from.
- If the session cannot run, for example because the node has no handoff route, the app falls back to the Access login page in the web view for the rest of the launch. Closing the sheet is not a failure: the app shows Reload.
- If Access asks for a login again within two minutes of a sign-in that worked, the token did not take and the login page loads in the web view, instead of looping.
- Changing the home node removes the Access cookie of the old node from the web view. The web client has no sign-out of its own. The session in Safari is separate and is left alone.

What it needs:

- **The node:** the `/app/handoff` and `/app/callback` routes and the association file, with the app identifier (`<team id>.<bundle id>`) in the node's configuration.
- **Cloudflare Access:** a bypass policy for the one path `/.well-known/apple-app-site-association`, because Apple's servers fetch the file without signing in. The Access application must not bind the cookie to the browser, because the token is made in Safari and used in the app.
- **The build:** `AGENCY_ASSOCIATED_HOST` in `Local.xcconfig`. The project's minimum iOS is 17.4, the first version with `ASWebAuthenticationSession` callbacks on an https address.

## Push notifications

The node sends a notification when a request starts waiting for the owner. The app side:

1. **Permission.** The app does not ask on the first launch. After the owner has signed in (the first time a page of the home node has loaded, which only happens past Access), it shows a one-line explanation and then the system prompt, which asks for alerts, sounds, and the badge on the app icon together (iOS does not ask again after the first answer, so the badge is part of the first request). "Not now" asks again after a week. A refusal in the system prompt is never asked again; the owner can change it in Settings. `PushPromptPolicy` holds the rule.
2. **Token.** When permission is granted, the app registers for remote notifications on every launch, because the token can change.
3. **Hand-over to the page.** The page registers the token with the node (`POST /api/push/tokens`, with the page's write token and the Access session), so the app never talks to the node itself. The app injects `window.agencyNative = { platform: "ios", push: null | { token, environment, label } }` before the page runs, only into the main frame of the home node's origin. When the state changes while the page is open, it sets the value again and dispatches `agency:native-push` with the new `push` as `detail`. `token` is the APNs device token in lowercase hex, `environment` is `sandbox` or `production`, and `label` is the device model (`iPhone`), never the device's name. The script is `NativeBridge` in `apple/AgencyKit`.
4. **Tap.** A notification carries `agency: { org, team, request }`. Tapping it opens `/desk?item=<org>:<team>:<request>` on the home node, which is the route the web client already has. Values that are not plain identifiers (letters, digits, `.`, `_`, `-`) give no link, and the app just opens.

5. **Badge.** The app icon shows how many requests wait for the owner. The node puts the count in `aps.badge` of each push (an integer: the requests addressed to the owner across all orgs and teams, counting the one the push is about), and a push that arrives while the app is open sets the badge too. While the page is open, it keeps the number current: it calls `window.agencyNative.setBadge(count)` whenever the count changes, so deciding a request lowers the badge without another push. The page calls it only when every team's list has loaded, because a count that is too small hides waiting requests. The app takes a whole number from 0 to 9999 from the main frame of the home node's origin, and ignores any other message (`NativeMessage`). Changing the home node sets the badge to 0. While the app is closed, the badge can be out of date if a request is decided somewhere else, until the next push or until the app opens.

`aps-environment` is `development` in Debug builds and `production` in Release builds (`APS_ENVIRONMENT` in `project.yml`). The same value goes to `Info.plist` as `AgencyAPNsEnvironment`, which is how the app tells the node which APNs host a token belongs to. TestFlight and App Store builds are Release, so they register as `production`; a build run from Xcode on a device registers as `sandbox`.

What has to be in place outside the repository:

- The App ID for the bundle ID has the Push Notifications capability. With automatic signing and the API key, `-allowProvisioningUpdates` normally enables it when the entitlement is present; check it on the first archive.
- The node has an APNs key (a `.p8` from the developer account), its key ID, the team ID, and the same bundle ID as the topic. These are credentials: keep them out of the repository.

## Protocol code in Swift

The native screens of agency#77 build and sign decisions in Swift. The `AgencyProtocol` module of `apple/AgencyKit` holds that code: canonical JSON, the signed text (`agency/v1/<kind>`, a newline, the canonical JSON), the decision envelope with the shape checks the node makes, and the key fingerprint with the pairing code and device id. Its tests read `testdata/vectors.json`, which the Rust side produces: each body is built in Swift and has to encode to the vector's canonical JSON, and the vector's Ed25519 signatures have to verify over the bytes Swift made. A change of the protocol that the Swift code does not follow fails those tests.

The `AgencyAPI` module is the client of the home node's Client API (`NodeClient`, with the transport behind a protocol) and the wire types it reads (`Me`, `TeamSnapshot` with its `TeamView`, `DecisionLog`, `DecisionResult`, `Session`). Each type decodes the node's real answer in `testdata/client/`, which the Rust tests record and fail on when it changes, so a field the node renames or drops fails a Swift test before it reaches a screen. A value the client does not know (a new request kind, a new decision state) fails to decode instead of being read as another. The client's rules:

- Every request carries the Access token as `cf-access-token`. A redirect is Access ending the session: the client throws `sessionExpired` and never follows it.
- A write also carries the write token from `GET /api/session`, fetched on first use and once more after a 403 (the token changes when the node restarts). A second 403 is the answer.
- The same call gives the node's time. A signed envelope's `at` comes from `nodeNow()`, because the node refuses an envelope more than five minutes ahead of its own clock.

Two rules in the encoder come from the protocol and are easy to break. Keys sort by their UTF-8 bytes, which is not Swift's string order and not UTF-16 order. Text is never normalized: Swift treats `é` and `e` plus U+0301 as the same string, but they are different signed bytes, so objects keep their members as a list and compare bytes.

## Not in the app yet

- Device key: the web client's device key lives in the web view's storage. Whether it survives app restarts is checked on the phone with the web client's `/devices` screen.
