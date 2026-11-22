// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "PearDebug",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "PearDebugKit", targets: ["PearDebugKit"]),
        .executable(name: "peardebug", targets: ["peardebug"]),
    ],
    targets: [
        .target(name: "PearDebugKit"),
        .executableTarget(name: "peardebug", dependencies: ["PearDebugKit"]),
        .testTarget(name: "PearDebugKitTests", dependencies: ["PearDebugKit"]),
    ]
)
