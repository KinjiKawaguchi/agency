# Agency for iPhone

`apple/` holds the iPhone app: native SwiftUI screens that talk to the owner's home node's Client API, and sign decisions with a key in the phone's Secure Enclave.

```
apple/
  AgencyKit/            Swift package: the protocol code, the Client API client, the screens' models and
                        wording, and the SwiftUI screens (AgencyUI). Everything but AgencyUI builds and
                        tests on macOS and Linux.
  Shared/               The app code every platform uses: first-launch setup, the session, the shell's
                        model and screens (`Shell/`), the sign-in, the Keychain, the Secure Enclave key, push,
                        the alert strings. Nothing here imports UIKit or AppKit.
  Mac/                  What is the Mac's alone: the app entry and delegate, the sidebar, the window the
                        sign-in sheet opens over, asking for a push token, the app icon, the sandbox
                        entitlements (the `AgencyMac` target).
  App/                  What is the iPhone's alone: the app entry and delegate, the tab bar, the window the
                        sign-in sheet opens over, asking for a push token, the app icon, Info.plist.
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
- Every screen is native and reads from the node's Client API (the next section). The app shows no web page. Earlier versions showed the web client in a web view; that web view, its cookie copy of the Access token and the bridge for the page were removed, and the data they left in the app (the cookie and the web client's own storage) is removed once at the first launch after the update (`LegacyWebData`).

## The native desk

- **The tabs** (five, the most a phone's bar shows without folding the rest into "More") are 決めること, 全体, 組織, 成果 and 端末, all native; 組織 holds the org chart and the participants and work behind a switch at the top, and 成果 holds the costs and the evaluation behind one, as the web client groups them. A screen is a case of the shell's `Tab` and of one switch (`screen(_:session:)`).
- **Calls to the node** go through `NodeClient` (`AgencyAPI`) over `URLSessionTransport`, which never follows a redirect: Access answers an ended session with one, and the app then runs the sign-in again. The Access token that the sign-in hands over is kept in the Keychain (`AccessTokenStore`, this phone only, not synced) and sent as `cf-access-token`.
- **The device key** is a P-256 key made inside the Secure Enclave (`SecureEnclaveSigner`). No process can read it. It signs only after Face ID, each time, and only while the current set of faces is enrolled: adding or removing a face destroys the key, and the phone is added again. The Keychain keeps an opaque handle to the key, not the key. A phone without a Secure Enclave or without Face ID set up cannot decide; it can still read. The settings can make the key again, for when a change of faces left the old one unusable. There is one key, one Keychain token and one set of drafts for each home node host, never shared between nodes, so a second home node later is a second session and not a rewrite.
- **Adding this phone.** A key signs decisions only once it is added to the org. The desk shows a card that says so, asks the node to add this phone (`POST /api/devices/requests`) and shows the pairing code (`xxxx-xxxx-xxxx`, from the key's fingerprint, the same as the node's). The owner compares it on a device that is already added, or at the node's terminal, and adds the phone there. Until then the desk can be read, options picked and text typed, and nothing can be sent. A browser's key (the web client's) is a different device; both can be added. While it waits, the card says exactly how it gets approved (agency#157): when the account already has devices, the labels of those devices ("open Agency on <labels> and approve it under Devices"), and when it has none, the command for the node's machine, which is the one the node gives in the device list (`node.command`, shown as text, never run, ignored when empty, over 500 characters or holding a control character) and otherwise `agency device-add`. The code is large, one line says what comparing it protects against, and the card asks the node again every 4 seconds, on the desk too, and goes away by itself once the phone is added.
- **The devices tab** (端末) shows this phone's pairing code and whether it is added, the person's devices (one row for a device in several orgs, the newest first), and the devices that asked to be added. Confirming one signs a device change (`DeviceChangeSender`): for each org that lacks the new device and holds this phone, a change `{ org, account, op: add, device, at }` is stamped with the node's time, signed with Face ID as this phone's own id in that org, and posted to `/api/devices/changes` (one Face ID prompt for each org). Before anything is signed the phone checks the request against its own key: the id and the code the node showed must be the ones the key makes, because a node that showed another code would have the person compare the wrong one and add a different key. Face ID is also the answer to adding a device from a phone: no other path signs for the person.
- **Sending** (`DeviceDecisionSender`): the envelope comes from the request's own `answerWith`, is stamped with the node's time from `GET /api/session`, is checked for the shape the node requires, and is signed after Face ID, which names the action and the request's title. The node's own device id for this key is read from the device list and used as the signer.
- **Push** is registered by the app itself (`POST /api/push/tokens`) once the desk loads, and the badge is the number of requests waiting on the owner. A tapped notification opens that request on the desk.
- **One shared read** (`SnapshotCache`): the desk, the overview, the participants and the costs screens all read the account and every team's view. They read through one cache in the session, so the app asks the node once for what several screens want and the screens do not disagree for a moment. A read within five seconds of the last is answered from it, two reads at the same moment cause one request, and a failed read is not kept. A decision drops what was read before it (`invalidate()`), and a read that started before the decision does not become the cache. A team that was read keeps its last answer on the desk when a later read of it fails, and its failure is still reported.
- **Settings** (the gear on the desk) show the home node and change it. Changing it forgets the token.

## The native overview

The overview tab mirrors the web client's Overview screen (`web/src/overview`): every org and team the account sees on one screen, with the funding sources' usage below. `AgencyModel/Overview` holds what it decides, ported from the web with its test cases:

- `TeamSummary` says what a team needs from the viewer: **mine** (the node's `mine` list is not empty, however busy the team is), **stuck** (work is blocked, counted by the reason the runtime gives), **idle** (participants, none working) or the rest. The viewer is not counted among the participants. Whose a request is stays the node's call.
- `OverviewOrder` puts teams in the order mine, stuck, unreadable, idle, the rest, still loading; within a rank the team with more waiting or more blocked comes first, then by name. An org stands where its most pressing team does.
- `WindowForecast` projects a funding window to its reset from the pace so far, and `FundingSummary` lists the weekly window before the five-hour one. A window whose reset has passed has no forecast.
- `OverviewStore` reads `GET /api/me`, each team's view and each readable team's usage over 7 days (`NodeClient.usage`), and keeps each team's own outcome: a team that cannot be read is shown as that, in its place, and a team that was read keeps its last answer when a later read fails. A team whose node cannot be reached is not asked for usage (a 404 means its runtime reports none).

The views are in `AgencyUI/Overview`; a long team name is cut after two lines and opens in full when tapped.

## The native participants and work

The tab mirrors the web client's participants and work screen (`web/src/people`), one section for each team the account can read: the participants without the viewer (face, what they do, since when, what they hold, and for someone who waits on the viewer, the request that stops them), the work in progress with its forecast, the work that cannot move with its reason, and the epics with their bars and forecasts. `AgencyModel/People` holds what it decides, ported with the web's test cases: `Work` (the bar of an epic, the order of epics, which work is in progress and which is blocked), `Identity` (the color and letters of a face, the same hash as the web), `PeopleSection` and its rows, and `PeopleStore`. Whether a participant waits on the viewer is the node's list (`waitingOnViewer`), as on the desk. A finished epic (`doneAt` present, or every child done) is shown as finished with the time it finished and no forecast, after the open epics. A long list shows its first rows (eight for work, six refs for a participant) and the rest open on a tap. `TeamReader` is the one place that reads every team's view for the overview and this screen.

## The native costs

The tab mirrors the web client's costs screen (`web/src/costs`): the funding sources, each with its weekly and five-hour window projected to its reset (the same cards as on the overview), then, for every team the account can read, its token use over 7, 14 or 30 days, priced as if billed by the API: three figures, the cost by day stacked by participant, and a card for each participant. `AgencyModel/Costs` holds what it decides, ported with the web's test cases: `UsageChart` (the series in the team's own order, each participant's fixed color slot with the ninth and later folded into "other", the columns and the totals), `TokenReport`, and `CostsStore` (a team whose runtime reports no usage answers 404 and says so; 503 says the team could not answer just now; any other failure is shown as that). A table of five columns would be wider than a phone, so each participant is a card. A tap on a day shows what each participant cost on it, and a long name is cut and opens in full when tapped.

## The native evaluation

The evaluation page of 成果 mirrors the web client's (`web/src/evaluation`): for the org chosen (the first, or one of several) and a period (14, 30 or 90 days), each team's headline numbers (new features and pull requests merged, the lead times with their median and p90, how long main was red, reverts, the cost per merged pull request), then each of them over the days: the merged pull requests stacked by type, the lead times and the cost per merged pull request as 7-day trends, main's red time and the reverts per day. Below, where each team's effort goes (`GET /api/orgs/<org>/effort`): its cost by work type as bars with the org's intended share marked on the same track and the gap in points, the last seven days against the weekly budget, the cost by day stacked by type, and the numbers in a table. The numbers are the node's: the screen only draws them, and the same route is what an agent cites. `AgencyAPI` reads the answers the way the web client does (`Evaluation`, `Effort`: a type left out counts as 0, a negative or non-numeric number refuses the whole answer, each team has its own `from` and `until` and is drawn against them), and the shared samples `testdata/evaluation.sample.json` and `testdata/effort.sample.json` are decoded in the tests. A pull request that reverts another counts in `reverts` and not in `merged`; the pull requests merged are both, as the node's `costPerMergedPrUsd` divides by. The org's plan is `teams[].plan` of the org definition (`OrgPlans`): only whole shares that sum to 100 and a whole budget are a plan, anything else is "no plan stated". A node that does not serve the routes yet answers 404 and the screen says so. A tap on a day opens what each series was on it. `AgencyModel/Evaluation` holds what it decides, ported with the web's test cases (`EvaluationSeries`, `DurationFormat`, `TeamEvaluationReport`, `EffortAnalysis`, `TeamEffortReport`, `EvaluationStore`).

## The native org chart

The org chart page of 組織 mirrors the web client's (`web/src/org`, the chart of every org of agency#145): with several orgs and none chosen, every org under the account, each with its departments (nested by parent), its teams and its members; with one org (or one chosen), that org's own chart, and a department opens its own chart ("back to the whole organization" returns). On a phone it is the indented list the web draws on a small screen. Each team's box carries the overview's signals, worked out by `TeamSummary` so the two screens cannot disagree: the requests that are the viewer's (the node's `mine`, rolled up into each department and org), stuck, idle, how many participants work, its participants as faces (six, then "+N"), and "old data" when its data is a copy that could not be renewed; a team that could not be read says so in its box, one still being read is grey bars. An org whose definition cannot be read is shown as such and the others are still drawn. A tap on a team opens the desk when it has requests waiting, else the overview. It reads `GET /api/me`, `GET /api/orgs/<org>` for every org (`OrgDefinition`) and the team views the overview reads (`TeamReader`, with the retry rule of the page above); a node route that combined them would save the phone those reads but would be a protocol addition. `AgencyModel/OrgChart` holds what it decides, ported with the web's test cases (`OrgTreeBuilder`, `OrgChartRow`, `TeamBoxText`, `OrgChartStore`).

## The Mac app

The `AgencyMac` target (macOS 14.4 or later, the same reason as iOS 17.4: the https callback of the sign-in session) is the same app on a Mac: the same session, screens, news, and push kinds from `Shared/` and the package, a sidebar instead of a tab bar (`Mac/HomeShell.swift`), and the sandbox of the Mac App Store, because it is distributed through TestFlight for Mac and not outside Apple (no Developer ID, no notarization). It has the iPhone's name and bundle ID, so it is one record in App Store Connect with the Mac added, and the node's association file (`access.appId`) and push topic need no change.

- **The key.** Each Mac makes its own P-256 key in the Secure Enclave, one for each home node, and is added as another device through the same pairing (approve it from a device that is already added). The rule differs from the iPhone's on purpose: the iPhone's key needs Face ID alone (`.biometryCurrentSet`); the Mac's key needs Touch ID **or the Mac's password** (`.userPresence`), because a Mac may have no Touch ID, or be used closed with an external keyboard, where a Touch-ID-only key could never sign. It is still a Secure Enclave key that cannot be read, and each signature still asks for the person. `.biometryCurrentSet` is not used there: Apple documents it as needing Touch ID to be available and enrolled, which is the case the rule exists for, and with the password as an alternative, tying the key to the current set of fingers would protect nothing. The settings and the confirmation lines say "Touch ID か Mac のパスワード" there.
- **Keychain.** The Mac uses the data protection keychain (`kSecUseDataProtectionKeychain`), which the sandboxed app reaches through its `keychain-access-groups` entitlement; the login keychain would ignore the accessibility class and ask the person for access.
- **Push and the Dock.** The same coordinator asks for permission and registers for remote notifications (`com.apple.developer.aps-environment` in the entitlements); the push label is `Mac`, never the computer's name. The badge on the Dock icon is the number of requests waiting.
- **Not on the Mac yet:** a menu-bar item, the Settings window on ⌘, (the settings are the sheet behind the gear, as on the iPhone), the widgets.

### Getting the Mac build to TestFlight

`apple/scripts/testflight.sh --platform macos` archives the `AgencyMac` scheme for macOS and sends it to the same app record as the iPhone's. What the App Store Connect API can and cannot do for the Mac is not fully documented by Apple, so `apple/scripts/register-bundle-id.sh` only *reports* what it finds about the Mac and changes the one thing it is told to. These steps are the owner's, in this order; the script prints which of them are still open:

1. **The bundle ID.** The script registers or finds the iPhone's bundle ID and its capabilities (Push Notifications, Associated Domains, App Groups). It then asks the API for a *macOS* bundle ID of the same name and says whether there is one. It never creates one: the API's `UNIVERSAL` platform value is documented but reported to be refused, and a second record is not what a universal purchase wants. If the first Mac archive fails with a signing error that names the identifier, open Certificates, Identifiers & Profiles, Identifiers in the developer portal, create the macOS identifier for the same string with the same capabilities (Push Notifications, Associated Domains), and keep the error text.
2. **The macOS platform of the app record.** In App Store Connect, open the app, choose Add Platform in the sidebar, then macOS. Or run the script again with `ADD_MACOS_PLATFORM=1` (it posts an App Store version with the platform `MAC_OS` and the app's own `MARKETING_VERSION` from `project.yml`, so no empty version of another number stays on the record; `ASC_MACOS_VERSION` names another). A read of the report that this key may not make is skipped with a message, and nothing is added when it could not look. **This cannot be undone:** once the app has two platform versions approved it is a universal purchase for good. That is why the script never does it without the flag, and why the owner decides.
3. **Certificates.** A Mac App Store build is signed with Apple Distribution and its installer package with Mac Installer Distribution. `-allowProvisioningUpdates` with the API key should create both; if the export says a certificate is missing, create it in the portal (Certificates, +, Mac Installer Distribution).
4. **The build.** `ASC_KEY_PATH=… ASC_KEY_ID=… ASC_ISSUER_ID=… apple/scripts/testflight.sh --platform macos` (the iPhone's is the same command without the option). The API cannot upload a build; the export step does.
5. **Install.** In App Store Connect, TestFlight, add yourself to an internal group for the macOS build; on the Mac, open the TestFlight app from the Mac App Store and install it.

What no script and no test here has seen, and the first build must answer (the owner's list): the sign-in sheet and its return to the app, the Touch ID or password prompt when the key is made and for each signature, adding this Mac from the iPhone (compare the code, approve), a banner within 30 seconds of something new, a push for a device or a refused decision and its tap, the Dock badge, the menu-bar count, ⌘, for settings, and quitting and opening again with the window closed.

## A read that is not ready yet

A node answers 503 for a team's view while its board is still being read (and for its usage when the team's runtime does not answer in time), and a slow node times out. The native screens that read team views (the overview, participants and work, costs) do not show that as a failure (agency#170, the same rule as the web client). `RetryPolicy` asks again after 1, 2, 4, 8 and 15 seconds for the answers that say "not now" (408, 429, 502, 503, 504, and a read with no answer at all); every other refusal, a 404 and the ended sign-in are final and shown at once. While a team is not ready its card is grey bars (a skeleton), and the other teams are shown as they arrive. What is on screen stays when a refresh fails. Only when the retries are used up does the region say what could not be read, why (the node's own words) and that it asks again 30 seconds later, with a "Try again" button; one region failing never blanks the screen. A screen that is shown reads again every 30 seconds.

## Nothing stands in for an answer (agency#303)

A screen never shows a value the node has not given: a `0`, an empty list or a default before the answer makes "not read yet" look like "really nothing". Until the node has answered, a screen shows grey bars (a skeleton) in the place of the content; when the read fails and nothing was read before, it says so and why. What was read before stays when a later read fails. The desk has `hasAnswer` (a team was read, or a read finished) and `knownWaitingCount` (`nil` until a read has finished, `0` only when the node said nothing waits); once a read has finished the desk stays answered, so a refresh under way or a failed one keeps the pile and the number; a place outside the window that shows the number (the Mac's menu-bar item, a badge) uses the optional one. What the node says about itself (`hidesTitles`, whether it sends the kinds of notification) is optional until it has said: the widgets show no title, and the settings claim nothing, until then. `NoStandInValuesTests` reads the sources of the screens, the widgets and the app for `?? 0`, `?? []`, `?? ""`, `?? false`, `?? true` and state that starts as a number or `true`, and fails for any that is not named in the test with the reason it is not a stand-in.

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
- `AGENCY_ASSOCIATED_HOST`: the host name of your home node. It goes into the app's associated domains and tells the app which node may use the passkey sign-in. Without it the app cannot sign in to the node (the desk says so).

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

**`apple/scripts/register-bundle-id.sh`** registers the bundle ID from `Local.xcconfig` for iOS if the account does not have it, and enables the Push Notifications, Associated Domains and App Groups capabilities on it; it does the same for the widget extension's bundle ID (the app's with `.widgets`), with App Groups. It signs the API token itself (ES256, valid for ten minutes) with `openssl`, and calls the API with `curl` and `jq`. It creates only what is missing, so it can be run again at any time, and it says what it did:

```
Registered bundle ID com.acme.agency.
Enabled Push Notifications.
Enabled Associated Domains.
Enabled App Groups.
Registered bundle ID com.acme.agency.widgets.
Enabled App Groups.
Next: the app group group.com.acme.agency must exist and be ticked for both bundle IDs. The API cannot create it.
```

The name of a new bundle ID is `Agency`, and of the widget extension's `Agency Widgets`; set `ASC_APP_NAME` and `ASC_WIDGET_NAME` to use others. The API cannot create the app group (`group.<bundle ID>`): make it once in the developer portal (Identifiers, App Groups), or let Xcode's automatic signing create it during the TestFlight build, which runs with the App Store Connect key.

**`apple/scripts/testflight.sh`** does everything for one build:

1. generates the Xcode project (`xcodegen generate`),
2. archives the Release configuration with a build number made from the current UTC time (`BUILD_NUMBER` overrides it; the number must be higher than the last one uploaded for this version),
3. exports with `ExportOptions.plist`, whose `destination` is `upload` and `method` is `app-store-connect`, so the export step sends the build to App Store Connect,
4. deletes `apple/build/` and this project's DerivedData folder, also when a step failed.

The build says which source it was made from (`AgencyBuildCommit` and `AgencyBuildDescribe` in its `Info.plist`: the commit, and `git describe` against the `apple-v` tags), and every upload that succeeded is appended to `apple/uploads.log` (not committed). See [releasing.md](releasing.md) for versions and tags.

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

The app signs in to Access in an `ASWebAuthenticationSession`. It runs in Safari's context, so passkeys and password managers work, and it needs no password prompt of its own. (A `WKWebView` could not run WebAuthn for the Access team domain, `<team>.cloudflareaccess.com`: iOS allows that only for a relying party that is one of the app's associated domains, and the file that says so would have to be served by Cloudflare.)

1. When the desk has no Access token yet, or the session ended (the node answers with a redirect to Access's login), the app starts the session on `https://<node host>/app/handoff?state=<state>`. The `state` is 256 random bits made for this sign-in. The first launch starts it by itself once.
2. Access signs the owner in and returns to the handoff route, which sits behind Access. The node answers with a redirect to `https://<node host>/app/callback#token=<Access token>&state=<state>`.
3. That address is a universal link, so iOS hands it only to the app that the node's `/.well-known/apple-app-site-association` file names. The app checks the host, the path, that `state` is its own, and that the token is a JWT that has not expired (`AccessHandoff` in `apple/AgencyKit`), then keeps the token in the Keychain.

The token lives in the URL fragment so that it never reaches a server or a log. A callback that does not belong to a running session is ignored.

Whether it can be used is decided by the build and by the node:

- The app uses it only when the home node is on `https` port 443 and its host is `AGENCY_ASSOCIATED_HOST`, because that is the only host a universal link can return from. Otherwise the screens say that the app cannot sign in to this node. Closing the sheet is not a failure.
- Changing the home node forgets the token of the old node. The session in Safari is separate and is left alone.

What it needs:

- **The node:** the `/app/handoff` and `/app/callback` routes and the association file, with the app identifier (`<team id>.<bundle id>`) in the node's configuration.
- **Cloudflare Access:** a bypass policy for the one path `/.well-known/apple-app-site-association`, because Apple's servers fetch the file without signing in. The Access application must not bind its token to the browser, because the token is made in Safari and used by the app.
- **The build:** `AGENCY_ASSOCIATED_HOST` in `Local.xcconfig`. The project's minimum iOS is 17.4, the first version with `ASWebAuthenticationSession` callbacks on an https address.

## Push notifications

The node sends a notification when a request starts waiting for the owner, when a device asks to be added, and when a decision of the owner's is refused, to the kinds each registered token wants. The app side:

1. **Permission.** The app does not ask on the first launch. After the owner has signed in (the first time the desk has loaded, which only happens past Access), it shows a one-line explanation and then the system prompt, which asks for alerts, sounds, and the badge on the app icon together (iOS does not ask again after the first answer, so the badge is part of the first request). "Not now" asks again after a week. A refusal in the system prompt is never asked again; the owner can change it in Settings. `PushPromptPolicy` holds the rule.
2. **Token.** When permission is granted, the app registers for remote notifications on every launch, because the token can change.
3. **Registration.** The app registers the token with the node itself (`POST /api/push/tokens`, with the write token and the Access token) once the desk has loaded. `token` is the APNs device token in lowercase hex, `environment` is `sandbox` or `production`, `label` is the device model (`iPhone`), never the device's name, and `events` is the kinds the owner chose in the settings (`request`, `device`, `decision`; every kind starts on). A change in the settings registers again, which the node takes as an update. A node that predates kinds answers 400 to `events` (it refuses a field it does not know) and sends requests only: the app registers again without `events`, and the settings say that this home node sends requests only.
4. **Tap.** A request's notification carries `agency: { org, team, request }`: tapping it opens that request on the desk (`<org>:<team>:<request>` is the id of a desk item). A device's carries `agency: { type: "device" }` and opens the devices tab; a refused decision's carries `agency: { type: "decision", org, team, id }` and opens the desk. Values that are not plain identifiers (letters, digits, `.`, `_`, `-`), a type the app does not know, and a payload without `agency` give no link, and the app just opens.
5. **Wording.** The alert of a device or a refused decision is worded by the app, from `loc-key` and `loc-args`: `Shared/ja.lproj/Localizable.strings` and `Shared/en.lproj/Localizable.strings` hold the five strings the node names (`PUSH_DEVICE_WAITING`, `PUSH_DEVICE_WAITING_NO_LABEL`, `PUSH_DECISION_REFUSED`, `PUSH_DECISION_REFUSED_NO_TITLE`, `PUSH_REQUEST_WAITING_HIDDEN`), and a test checks that both languages have exactly those and the number of arguments the node sends. The development language is Japanese.

6. **Badge.** The app icon shows how many requests wait for the owner. The node puts the count in `aps.badge` of each push (an integer: the requests addressed to the owner across all orgs and teams, counting the one the push is about), and a push that arrives while the app is open sets the badge too. While the app is open, the desk keeps the number current (the sum of `mine` over the teams it could read), so deciding a request lowers the badge without another push. Changing the home node sets the badge to 0. While the app is closed, the badge can be out of date if a request is decided somewhere else, until the next push or until the app is opened.

`aps-environment` is `development` in Debug builds and `production` in Release builds (`APS_ENVIRONMENT` in `project.yml`). The same value goes to `Info.plist` as `AgencyAPNsEnvironment`, which is how the app tells the node which APNs host a token belongs to. TestFlight and App Store builds are Release, so they register as `production`; a build run from Xcode on a device registers as `sandbox`.

What has to be in place outside the repository:

- The App ID for the bundle ID has the Push Notifications capability. With automatic signing and the API key, `-allowProvisioningUpdates` normally enables it when the entitlement is present; check it on the first archive.
- The node has an APNs key (a `.p8` from the developer account), its key ID, the team ID, and the same bundle ID as the topic. These are credentials: keep them out of the repository.

## The widgets

A widget shows what waits on the owner: the number of requests that are the viewer's to decide (the desk tab's number) and the first three of them, each with its team. The home screen has a small and a medium widget; the lock screen has a circular one (the number), a rectangular one (the number and the first request) and an inline one ("決めること N 件"). A tap opens the desk at the first request; on the medium widget each request opens its own.

- **The data.** The app writes one small file, `widget.json`, into the app group's container after each desk read, and removes it when the home node is left. It holds a version, when it was written, the home node's host, the count, and for each of the first three requests its org, team and request ids, its kind, its title (cut to 80 characters) and its team's label: no people, no bodies, no options, no token and no key. The widget never holds the Access token or a device key. The file is readable after the first unlock (the widget reads it while the phone is locked) and written whole or not at all; a file of another version, or one that is not a snapshot or is far too large, is ignored.
- **Privacy.** Titles are `privacySensitive()`: on the lock screen the system hides them and the count stays.
- **Titles off.** A home-screen widget shows titles to whoever sees the phone (and in StandBy, while it is locked). The owner can turn "ウィジェットにタイトルを表示" off in the settings (on by default, kept per home node on this device): the file is then written without titles and team labels, so only the count and the kind of each request are on the device at all, and the widget shows those. A node that sets `hideTitles` (see `docs/protocol.md`, Push notifications) is to say so in its answer to `GET /api/me` (`hidesTitles`; the node does not send it yet, agency#272, and the app counts a missing field as `false`); the file is then written without titles whatever the switch says. The switch can only hide more, never show what the node hides, and the settings row says that the node hides them.
- **While the app is closed** (agency#261). The node wakes the app with a push that has only `content-available` (and a request's alert carries it too), at most once in 15 minutes (docs/protocol.md, "Waking the app to read the desk"). The app declares the `remote-notification` background mode, and `AppDelegate.application(_:didReceiveRemoteNotification:)` reads the desk with no window and no session (`BackgroundDeskRefresh`: the saved home node, the Access token from the Keychain, which is readable after the first unlock, one read of the account and every team without waiting for a team that is not ready, then the same file and the same rules about titles). It does nothing when the app is in front, and not more than once a minute; it stops after 25 seconds, a little before the system's limit. A run that cannot read the whole desk (the sign-in ended, a team did not answer) writes nothing, because a number made of the teams that answered would be wrong; the widget keeps its file and says how old it is. iOS may throttle or skip a background push: the widget is then as late as it was before.
- **When the file is removed.** On leaving the home node, and when the device's key is reset (a reset means this device is no longer trusted with that home node). An ended sign-in keeps it: the three-hour mark on the widget covers that.
- **`AgencyWidget`** is a package target that depends only on `AgencyKit`: the snapshot, its file store, the link, the words and the SwiftUI views. A widget extension has a small memory limit, so it must not link `AgencyAPI`, `AgencyProtocol` or swift-crypto. `AgencyModel` depends on it and holds the builder that works the snapshot out of the desk (`WidgetSnapshotBuilder`). The views are plain SwiftUI (no WidgetKit names), so they are drawn offscreen at the widgets' real sizes in the tests, and the tests fail when a widget would need more height than it has.
- **The tap.** The widget's URL is `agency://request?org=..&team=..&request=..` (`WidgetLink`). It only says where to go, and the app accepts it only when all three values look like identifiers (`RequestLink`). No URL scheme is registered in `Info.plist`: Apple's documentation says the system activates the containing app and passes a widget's URL to `onOpenURL`, and says nothing of registering it, so no other app or web page can open the app with it. This could not be tried on a device from here: if a tap only opens the app, the fallback is one `CFBundleURLTypes` entry for `agency`.
- **Refresh.** The widget shows the file as it is and says how old it is once it is older than three hours; the app asks the system to reload the widgets after it writes. The system budgets reloads, so the widget can be later than the app. A silent push from the node, with the remote-notification background mode, would refresh it while the app is closed; that is not built (agency#207).
- **The targets.** `project.yml` has the extension `AgencyWidgets` (`apple/Widgets`: the widget bundle, the timeline provider that reads the file, the entitlements and an `Info.plist` that is generated). Its bundle ID is the app's with `.widgets` (`$(inherited).widgets`, so it follows `PRODUCT_BUNDLE_IDENTIFIER` in `Local.xcconfig`), and it is embedded in the app. Both carry the app group `group.<the app's bundle ID>` in their entitlements (the extension's is `group.$(PRODUCT_BUNDLE_IDENTIFIER:base)`, the build setting with its last suffix removed), and the same name in `Info.plist` as `AgencyAppGroup`, which the code reads: the extension links only `AgencyKit` and `AgencyWidget`. The app writes the file (`WidgetPublisher` in `AgencyModel`, with the app group's container and WidgetKit behind a small protocol; `ShellModel` calls it after the desk read and `ShellChrome` on each change of the desk's requests, so it does not depend on which screen is open; a write that would say the same as the last is skipped, because the system budgets reloads) and removes it when the home node is left, after which that publisher writes nothing again (a read that was under way cannot bring the old desk back); a tap's URL goes to `onOpenURL` in `AgencyApp`, which opens the request like a tapped notification.
- **The Mac.** The same extension can be built for macOS 14 (WidgetKit runs there), reading the same snapshot from the Mac app's group container; `AgencyWidget` has no iOS-only code except the file protection class of the written file.

## What is new

The app and the web client use one definition of "what is new" (agency#167), pinned by `testdata/notifications.json`. A client keeps the last answer it read for each team, for the list of devices that asked to be added, and for the decision log. An event is the difference between that answer and the next one; the first answer of each is a baseline and makes no event, and a part that could not be read keeps its last answer, so the read after a failure is not "everything is new". The events are a request that now waits on the viewer (`mine`), a participant who now waits on the viewer, a work item that is newly blocked, a device that asks to be added (other than this phone's own key, and only when this phone can approve one), and a decision that changed state (a success this phone sent is not news, a failure or a queued one is). Each has a stable key (`request:org/team/id`, `participant:org/team/id`, `blocked:org/team/ref`, `device:<key>`, `decision:id:state`), and the events of one reading come ordered by kind and then by key. `EventEngine` in `AgencyModel` is the Swift side; the web client's engine reads the same file and has to make the same events. The counts (the badge, the mark on the devices tab) are not events: they are the current number.

### What the owner chooses

The settings sheet (the gear on the desk) has two sections, for each home node: which kinds of news show a banner while the app is open (all five), and which kinds are sent as a notification while it is closed (request, device, refused decision). Every kind starts on. The choices are kept in the app's defaults per home node (`agency.notifications.<host>`).

### The news, as banners

`NewsStore` (`AgencyModel`) turns the events of that definition into what the app shows while it is open. Each `poll()` reads the account, every team (through the shared cache), the devices and the decision log, gives the reading to `EventEngine`, and makes at most three banners, one for each kind of event: worded for one event ("<team>: 「<title>」があなたを待っています") or for several ("3 件の依頼があなたを待っています"), and leading somewhere (a request opens on the desk, a device to the devices screen, a participant or a blocked item to the people screen). A kind the owner turned off, or one the visible screen already shows (the devices while the devices screen is open), makes no banner, and its events are still taken so they are not announced later. A decision this phone sent is remembered for the session (`SentDecisions`), so its success is not news and its failure is.

The shell polls the news while the app is in front: once when it comes to the front (which is also when events that piled up while it was away arrive, together, in order) and every 30 seconds after that. The banners (`NewsBannerStack`) sit at the top, last 7 seconds, and have a cross; a long text wraps to three lines and the full text is on the screen the banner leads to. The first reading after launch is a baseline, so opening the app announces nothing that was already there.

## Protocol code in Swift

The native screens of agency#77 build and sign decisions in Swift. The `AgencyProtocol` module of `apple/AgencyKit` holds that code: canonical JSON, the signed text (`agency/v1/<kind>`, a newline, the canonical JSON), the decision envelope with the shape checks the node makes, and the key fingerprint with the pairing code and device id. Its tests read `testdata/vectors.json`, which the Rust side produces: each body is built in Swift and has to encode to the vector's canonical JSON, and the vector's Ed25519 signatures have to verify over the bytes Swift made. A change of the protocol that the Swift code does not follow fails those tests.

The `AgencyAPI` module is the client of the home node's Client API (`NodeClient`, with the transport behind a protocol) and the wire types it reads (`Me`, `TeamSnapshot` with its `TeamView`, `DecisionLog`, `DecisionResult`, `Session`). Each type decodes the node's real answer in `testdata/client/`, which the Rust tests record and fail on when it changes, so a field the node renames or drops fails a Swift test before it reaches a screen. A value the client does not know (a new request kind, a new decision state) fails to decode instead of being read as another. The client's rules:

- Every request carries the Access token as `cf-access-token`. A redirect is Access ending the session: the client throws `sessionExpired` and never follows it.
- A write also carries the write token from `GET /api/session`, fetched on first use and once more after a 403 that says the token is missing or wrong (it changes when the node restarts). Any other refusal, and a second one, is the answer and is not sent again.
- The same call gives the node's time. A signed envelope's `at` comes from `nodeNow()`, because the node refuses an envelope more than five minutes ahead of its own clock.

The `AgencyModel` module is what the screens show, ported from the web client with its test cases so the phone makes the same rules: the desk's pile (groups proposal, question, pending decision, and requests for someone else; the request that stops more people first; then the oldest), presence and the one-line summary, and the owner's replies. Whose a request is, is the node's call (`mine`); the model never works it out from `to`.

A reply is one of three things. **Choosing** an option decides the request, with an optional note. **Commenting** is text with no option on a request that has options: the team gets the message and nothing is decided, and the screen says so. **Answering** is words on a request that has no options. The title and the chosen option's label that go to the decision log are cut to 200 characters (Unicode scalars, as the node counts), because the node refuses a longer note.

The `AgencyUI` module holds the SwiftUI screens (iOS and macOS, nothing UIKit-only), and the words are in `AgencyModel` (`Ja`, taken from the web client's Japanese where it has the text), so they are tested on every platform. `DeskStore` is the desk's state: it reads through `NodeClient`, keeps drafts per request across launches, and sends a prepared reply through a sender that signs it. `ReplyControls` decides what can be pressed, so the views only draw it. The rules of the layout come from the owner: no screen scrolls sideways, a long title or id wraps or is cut with the full text one tap away, and nothing explains a keyboard.

The Linux CI job does not build `AgencyUI`. A change to it is checked on a Mac before it is merged: `xcodebuild -scheme AgencyUI -destination 'generic/platform=iOS' CODE_SIGNING_ALLOWED=NO build` in `apple/AgencyKit`, and `swift test`, whose rendering tests draw each screen at a phone's width with `ImageRenderer` (offscreen, nothing launches) and fail when anything is wider than the screen. Set `AGENCY_SNAPSHOT_DIR` to a folder outside the repository to keep the PNGs. They are made from fixtures only.

Two rules in the encoder come from the protocol and are easy to break. Keys sort by their UTF-8 bytes, which is not Swift's string order and not UTF-16 order. Text is never normalized: Swift treats `é` and `e` plus U+0301 as the same string, but they are different signed bytes, so objects keep their members as a list and compare bytes.
