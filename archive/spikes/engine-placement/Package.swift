// swift-tools-version: 6.0
import PackageDescription
let package = Package(
  name: "EnginePlacementSpike", platforms: [.macOS(.v15)],
  products: [.executable(name: "engine-placement", targets: ["Harness"])],
  dependencies: [.package(url: "https://github.com/loro-dev/loro-swift.git", exact: "1.16.2"), .package(path: "../placement-support")],
  targets: [
    .target(name: "SpikeCore", dependencies: [.product(name: "Loro", package: "loro-swift"), .product(name: "PlacementSupport", package: "placement-support")]),
    .executableTarget(name: "Harness", dependencies: ["SpikeCore"], linkerSettings: [.linkedFramework("AppKit"), .linkedFramework("WebKit")]),
    .testTarget(name: "SpikeTests", dependencies: ["SpikeCore"]),
  ], swiftLanguageModes: [.v5]
)
