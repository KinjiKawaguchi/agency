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

## Not in the app yet

- Push notifications: the `aps-environment` entitlement and the node side are a separate change.
- Device key: the web client's device key lives in the web view's storage. Whether it survives app restarts is checked in a separate change.
