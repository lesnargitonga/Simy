// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "SimySecurity",
    platforms: [
        .iOS(.v16),
        .macOS(.v13),
    ],
    products: [
        .library(
            name: "SimySecurity",
            targets: ["SimySecurity"]
        ),
    ],
    targets: [
        .target(
            name: "SimySecurity",
            path: "Sources/SimySecurity"
        ),
        .testTarget(
            name: "SimySecurityTests",
            dependencies: ["SimySecurity"],
            path: "Tests/SimySecurityTests"
        ),
    ]
)
