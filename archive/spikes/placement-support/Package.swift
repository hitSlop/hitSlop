// swift-tools-version: 6.0
import PackageDescription
let package = Package(name: "PlacementSupport", platforms: [.macOS(.v14)], products: [.library(name: "PlacementSupport", targets: ["PlacementSupport"])], targets: [.target(name: "PlacementSupport", linkerSettings: [.linkedLibrary("sqlite3")])], swiftLanguageModes: [.v5])
