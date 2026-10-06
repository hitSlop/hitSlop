// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "HitSlopApple",
  platforms: [.macOS(.v15)],
  products: [
    .library(name: "HitSlopCore", targets: ["HitSlopCore"]),
    .library(name: "HitSlopFirebase", targets: ["HitSlopFirebase"]),
    .library(name: "HitSlopDocument", targets: ["HitSlopDocument"]),
    .library(name: "HitSlopHost", targets: ["HitSlopHost"]),
    .library(name: "HitSlopCatalog", targets: ["HitSlopCatalog"]),
    .executable(name: "hitslop-native", targets: ["HitSlopNativeCLI"]),
  ],
  dependencies: [
    .package(url: "https://github.com/firebase/firebase-ios-sdk.git", exact: "12.19.2"),
    .package(url: "https://github.com/apple/swift-argument-parser.git", exact: "1.8.2"),
  ],
  targets: [
    .binaryTarget(name: "HitSlopCoreFFI", path: "Generated/HitSlopCoreFFI.xcframework"),
    // UniFFI's generated callback vtables predate Swift 6 concurrency checking. The core's
    // store links the platform SQLite.
    .target(name: "HitSlopCoreBinding", dependencies: ["HitSlopCoreFFI"], path: "Generated/HitSlopCoreBinding",
      swiftSettings: [.swiftLanguageMode(.v5)], linkerSettings: [.linkedLibrary("sqlite3")]),
    .target(name: "HitSlopDocument", dependencies: ["HitSlopCore", "HitSlopCoreBinding"], resources: [.copy("Resources/shell")], linkerSettings: [.linkedFramework("WebKit")]),
    .target(name: "HitSlopTestSupport", dependencies: ["HitSlopCore", "HitSlopCoreBinding", "HitSlopDocument"], path: "Tests/HitSlopTestSupport"),
    .testTarget(name: "HitSlopDocumentTests", dependencies: ["HitSlopDocument", "HitSlopCoreBinding", "HitSlopTestSupport"]),
    .target(
      name: "HitSlopCore",
      // The core parses window silhouettes, so manifest geometry has one parser.
      dependencies: ["HitSlopCoreBinding"],
      linkerSettings: [.linkedFramework("ImageIO")]
    ),
    .target(
      name: "HitSlopFirebase",
      dependencies: [
        "HitSlopCore",
        .product(name: "FirebaseCore", package: "firebase-ios-sdk"),
        .product(name: "FirebaseAnalytics", package: "firebase-ios-sdk"),
        .product(name: "FirebaseCrashlytics", package: "firebase-ios-sdk"),
      ]
    ),
    .target(
      name: "HitSlopHost",
      dependencies: ["HitSlopCore", "HitSlopDocument"],
      linkerSettings: [.linkedFramework("AppKit"), .linkedFramework("WebKit")]
    ),
    .target(name: "HitSlopFeatures", dependencies: ["HitSlopCore"]),
    .testTarget(name: "HitSlopFeaturesTests", dependencies: ["HitSlopFeatures"]),
    .target(
      name: "HitSlopCatalog",
      dependencies: [
        "HitSlopHost", "HitSlopCore", "HitSlopDocument", "HitSlopFeatures",
      ],
      resources: [.process("Resources")],
      linkerSettings: [.linkedFramework("AppKit")]
    ),
    .executableTarget(
      name: "HitSlopNativeCLI",
      dependencies: [
        "HitSlopCore",
        "HitSlopHost",
        "HitSlopDocument",
        .product(name: "ArgumentParser", package: "swift-argument-parser"),
      ],
      linkerSettings: [.linkedFramework("AppKit")]
    ),
    .testTarget(name: "HitSlopCoreTests", dependencies: ["HitSlopCore", "HitSlopCoreBinding", "HitSlopDocument", "HitSlopTestSupport"]),
    .testTarget(name: "HitSlopHostTests", dependencies: ["HitSlopHost", "HitSlopCore", "HitSlopCoreBinding", "HitSlopDocument", "HitSlopTestSupport"]),
    .testTarget(
      name: "HitSlopCatalogTests",
      dependencies: ["HitSlopCatalog", "HitSlopCore", "HitSlopDocument", "HitSlopFeatures", "HitSlopTestSupport"]
    ),
  ],
  swiftLanguageModes: [.v6]
)
