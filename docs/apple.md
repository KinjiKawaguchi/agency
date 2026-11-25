# Agency for iPhone

`apple/` holds a thin iPhone app. It opens the owner's home node in a web view, so everything the web client does works in it. Native features (push notifications, a device key held natively) are added on top, one at a time.

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
```

## How the app behaves

- On the first launch it asks for the home node address and keeps it in `UserDefaults`. Only the origin is kept: a pasted path is dropped. Plain `http` is accepted only for `localhost`, `127.0.0.1`, and `.local` names, so a node on the owner's machine can be tried.
- It loads the node in a `WKWebView` that uses the default website data store, which is persistent. The Cloudflare Access session cookie and the web client's own storage survive app launches. How long the sign-in lasts is the session duration of the Access application.
- Only the home node and `<team>.cloudflareaccess.com` (https) load inside the app. Any other `http(s)` address opens in Safari, `mailto:` and `tel:` go to the system, and every other scheme is ignored. A link with `target="_blank"` follows the same rule instead of opening a window. The rule is `NavigationPolicy` in `apple/AgencyKit`.
- Sign-in through an identity provider other than the Access one-time PIN needs the provider's host on that allow list, or its pages open in Safari and the session never reaches the app.
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

`Local.xcconfig` is ignored by git. Without it the project builds with a placeholder bundle ID, which cannot be signed.

## Archive and send to TestFlight

Once per app:

1. Create the app record in App Store Connect with the bundle ID from `Local.xcconfig`.
2. Create an App Store Connect API key (Users and Access, Integrations) with the App Manager role, and keep the `.p8` file out of the repository. Note the key ID and the issuer ID.

For every build (the build number must be higher than the last one uploaded for this version, so derive it from the time):

```sh
cd apple
xcodegen generate

KEY_PATH=~/path/to/AuthKey_XXXXXXXXXX.p8   # the API key file
KEY_ID=XXXXXXXXXX                          # its key ID
ISSUER_ID=00000000-0000-0000-0000-000000000000
AUTH=(-allowProvisioningUpdates
      -authenticationKeyPath "$KEY_PATH"
      -authenticationKeyID "$KEY_ID"
      -authenticationKeyIssuerID "$ISSUER_ID")

xcodebuild archive -project Agency.xcodeproj -scheme Agency -configuration Release \
  -destination 'generic/platform=iOS' -archivePath build/Agency.xcarchive \
  CURRENT_PROJECT_VERSION="$(date +%Y%m%d%H%M)" "${AUTH[@]}"

xcodebuild -exportArchive -archivePath build/Agency.xcarchive \
  -exportOptionsPlist ExportOptions.plist -exportPath build/export "${AUTH[@]}"
```

`-allowProvisioningUpdates` with the API key lets Xcode create the certificate and provisioning profile itself. With `destination` set to `upload` in `ExportOptions.plist`, the export step sends the build to App Store Connect. To keep an `.ipa` instead, set it to `export` and upload `build/export/Agency.ipa` with the Transporter app.

After processing (a few minutes), the build appears under TestFlight in App Store Connect. Add the owner as an internal tester. Internal testing needs no beta review.

`MARKETING_VERSION` (the version the owner sees) is set in `project.yml`.

Delete `build/` and the DerivedData folder afterwards: the archive and its symbols are large.

## Push notifications

The node sends a notification when a request starts waiting for the owner. The app side:

1. **Permission.** The app does not ask on the first launch. After the owner has signed in (the first time a page of the home node has loaded, which only happens past Access), it shows a one-line explanation and then the system prompt. "Not now" asks again after a week. A refusal in the system prompt is never asked again; the owner can change it in Settings. `PushPromptPolicy` holds the rule.
2. **Token.** When permission is granted, the app registers for remote notifications on every launch, because the token can change.
3. **Hand-over to the page.** The page registers the token with the node (`POST /api/push/tokens`, with the page's write token and the Access session), so the app never talks to the node itself. The app injects `window.agencyNative = { platform: "ios", push: null | { token, environment, label } }` before the page runs, only into the main frame of the home node's origin. When the state changes while the page is open, it sets the value again and dispatches `agency:native-push` with the new `push` as `detail`. `token` is the APNs device token in lowercase hex, `environment` is `sandbox` or `production`, and `label` is the device model (`iPhone`), never the device's name. The script is `NativeBridge` in `apple/AgencyKit`.
4. **Tap.** A notification carries `agency: { org, team, request }`. Tapping it opens `/desk?item=<org>:<team>:<request>` on the home node, which is the route the web client already has. Values that are not plain identifiers (letters, digits, `.`, `_`, `-`) give no link, and the app just opens.

`aps-environment` is `development` in Debug builds and `production` in Release builds (`APS_ENVIRONMENT` in `project.yml`). The same value goes to `Info.plist` as `AgencyAPNsEnvironment`, which is how the app tells the node which APNs host a token belongs to. TestFlight and App Store builds are Release, so they register as `production`; a build run from Xcode on a device registers as `sandbox`.

What has to be in place outside the repository:

- The App ID for the bundle ID has the Push Notifications capability. With automatic signing and the API key, `-allowProvisioningUpdates` normally enables it when the entitlement is present; check it on the first archive.
- The node has an APNs key (a `.p8` from the developer account), its key ID, the team ID, and the same bundle ID as the topic. These are credentials: keep them out of the repository.

## Not in the app yet

- Device key: the web client's device key lives in the web view's storage. Whether it survives app restarts is checked on the phone with the web client's `/devices` screen.
