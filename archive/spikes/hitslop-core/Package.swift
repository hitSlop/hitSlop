// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "HitSlopCoreSpike",
  platforms: [.macOS(.v15)],
  products: [.executable(name: "core-conformance", targets: ["CoreConformance"]), .library(name: "NativeOwner", targets: ["NativeOwner"]), .executable(name: "rust-placement", targets: ["NativePlacement"]), .executable(name: "slop-spike", targets: ["SlopSpike"]), .executable(name: "owner-host", targets: ["OwnerHost"])],
  dependencies: [.package(path: "../placement-support")],
  targets: [
    .executableTarget(name: "NativePlacement", dependencies: ["NativeOwner"], path: "Sources/Placement", swiftSettings: [.define("RUST_CORE"), .swiftLanguageMode(.v5)], linkerSettings: [.linkedFramework("AppKit"), .linkedFramework("WebKit")]),
    .target(name: "NativeOwner", dependencies: ["HitSlopCoreBinding", .product(name: "PlacementSupport", package: "placement-support")]),
    .testTarget(name: "NativeOwnerTests", dependencies: ["NativeOwner"]),
    .target(name: "OwnerService", dependencies: ["NativeOwner"], swiftSettings: [.swiftLanguageMode(.v5)]),
    .executableTarget(name: "SlopSpike", dependencies: ["OwnerService"], swiftSettings: [.swiftLanguageMode(.v5)]),
    .executableTarget(name: "OwnerHost", dependencies: ["OwnerService"], swiftSettings: [.swiftLanguageMode(.v5)]),
    .binaryTarget(name: "HitSlopCoreSpikeFFI", path: "dist/HitSlopCoreSpikeFFI.xcframework"),
    .target(name: "HitSlopCoreBinding", dependencies: ["HitSlopCoreSpikeFFI"], path: "dist/bindings"),
    .executableTarget(name: "CoreConformance", dependencies: ["HitSlopCoreBinding"], path: "Sources/Conformance"),
  ]
)

