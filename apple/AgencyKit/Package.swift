// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AgencyKit",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "AgencyKit", targets: ["AgencyKit"]),
    ],
    targets: [
        .target(name: "AgencyKit"),
        .testTarget(name: "AgencyKitTests", dependencies: ["AgencyKit"]),
    ]
)
