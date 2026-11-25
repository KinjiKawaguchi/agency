// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AgencyKit",
    platforms: [.iOS("17.4"), .macOS(.v14)],
    products: [
        .library(name: "AgencyKit", targets: ["AgencyKit"]),
        .library(name: "AgencyProtocol", targets: ["AgencyProtocol"]),
        .library(name: "AgencyAPI", targets: ["AgencyAPI"]),
    ],
    dependencies: [
        // CryptoKit's API on every platform, so the signing code is tested on Linux too.
        .package(url: "https://github.com/apple/swift-crypto.git", from: "3.0.0"),
    ],
    targets: [
        .target(name: "AgencyKit"),
        .testTarget(name: "AgencyKitTests", dependencies: ["AgencyKit"]),
        .target(name: "AgencyProtocol", dependencies: [.product(name: "Crypto", package: "swift-crypto")]),
        .target(name: "AgencyAPI", dependencies: ["AgencyProtocol"]),
        .testTarget(name: "AgencyAPITests", dependencies: ["AgencyAPI", "AgencyProtocol"]),
        .testTarget(name: "AgencyProtocolTests", dependencies: ["AgencyProtocol", .product(name: "Crypto", package: "swift-crypto")]),
    ]
)
