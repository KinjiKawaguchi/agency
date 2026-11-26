// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AgencyKit",
    platforms: [.iOS("17.4"), .macOS(.v14)],
    products: [
        .library(name: "AgencyKit", targets: ["AgencyKit"]),
        .library(name: "AgencyProtocol", targets: ["AgencyProtocol"]),
        .library(name: "AgencyAPI", targets: ["AgencyAPI"]),
        .library(name: "AgencyModel", targets: ["AgencyModel"]),
        .library(name: "AgencyUI", targets: ["AgencyUI"]),
        .library(name: "AgencyWidget", targets: ["AgencyWidget"]),
    ],
    dependencies: [
        // CryptoKit's API on every platform, so the signing code is tested on Linux too.
        .package(url: "https://github.com/apple/swift-crypto.git", from: "3.15.1"),
    ],
    targets: [
        .target(name: "AgencyKit"),
        .testTarget(name: "AgencyKitTests", dependencies: ["AgencyKit"]),
        .target(name: "AgencyProtocol", dependencies: [.product(name: "Crypto", package: "swift-crypto")]),
        .target(name: "AgencyAPI", dependencies: ["AgencyProtocol"]),
        // What a widget shows and how it is written and read: no dependency but AgencyKit, because a widget extension has a
        // small memory limit and must not link the API client or the signing code.
        .target(name: "AgencyWidget", dependencies: ["AgencyKit"]),
        .testTarget(name: "AgencyWidgetTests", dependencies: ["AgencyWidget", "AgencyKit"]),
        .target(name: "AgencyModel", dependencies: ["AgencyAPI", "AgencyProtocol", "AgencyWidget"]),
        .target(name: "AgencyUI", dependencies: ["AgencyModel", "AgencyAPI", "AgencyProtocol"]),
        .testTarget(name: "AgencyUITests", dependencies: ["AgencyUI", "AgencyModel", "AgencyAPI", "AgencyWidget"]),
        .testTarget(name: "AgencyModelTests", dependencies: ["AgencyModel", "AgencyAPI", "AgencyProtocol"]),
        .testTarget(name: "AgencyAPITests", dependencies: ["AgencyAPI", "AgencyProtocol"]),
        .testTarget(name: "AgencyProtocolTests", dependencies: ["AgencyProtocol", .product(name: "Crypto", package: "swift-crypto")]),
    ]
)
