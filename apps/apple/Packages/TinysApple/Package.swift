// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "TinysApple",
  platforms: [.macOS(.v15), .iOS("26.0")],
  products: [
    .library(name: "TinysCore", targets: ["TinysCore"]),
    .library(name: "TinysFirebase", targets: ["TinysFirebase"]),
    .library(name: "TinysAccount", targets: ["TinysAccount"]),
    .library(name: "TinysDocument", targets: ["TinysDocument"]),
    .library(name: "TinysHost", targets: ["TinysHost"]),
    .library(name: "TinysCatalog", targets: ["TinysCatalog"]),
    .library(name: "TinysWidgets", targets: ["TinysWidgets"]),
  ],
  dependencies: [
    .package(url: "https://github.com/firebase/firebase-ios-sdk.git", exact: "12.19.2")
  ],
  targets: [
    .binaryTarget(name: "TinysCoreFFI", path: "Generated/TinysCoreFFI.xcframework"),
    // UniFFI's generated callback vtables predate Swift 6 concurrency checking. The core's
    // store links the platform SQLite.
    .target(
      name: "TinysCoreBinding", dependencies: ["TinysCoreFFI"], path: "Generated/TinysCoreBinding",
      swiftSettings: [.swiftLanguageMode(.v5)], linkerSettings: [.linkedLibrary("sqlite3")]),
    .target(
      name: "TinysDocument", dependencies: ["TinysCore", "TinysCoreBinding", "TinysAccount", "TinysWidgets"],
      resources: [.copy("Resources/shell")], linkerSettings: [.linkedFramework("WebKit")]),
    .target(
      name: "TinysTestSupport", dependencies: ["TinysCore", "TinysCoreBinding", "TinysDocument"],
      path: "Tests/TinysTestSupport"),
    .testTarget(
      name: "TinysDocumentTests", dependencies: ["TinysDocument", "TinysCoreBinding", "TinysTestSupport"]),
    .target(
      name: "TinysCore",
      // The core parses window silhouettes, so manifest geometry has one parser.
      dependencies: ["TinysCoreBinding"],
      resources: [.process("Resources")],
      linkerSettings: [.linkedFramework("ImageIO"), .linkedFramework("CloudKit")]
    ),
    .target(
      name: "TinysAccount",
      dependencies: [
        "TinysCore", "TinysCoreBinding",
        .product(name: "FirebaseCore", package: "firebase-ios-sdk"),
        .product(name: "FirebaseAuth", package: "firebase-ios-sdk"),
      ]
    ),
    .target(
      name: "TinysFirebase",
      dependencies: [
        "TinysCore",
        .product(name: "FirebaseCore", package: "firebase-ios-sdk"),
        .product(name: "FirebaseAnalytics", package: "firebase-ios-sdk"),
        .product(name: "FirebaseCrashlytics", package: "firebase-ios-sdk"),
      ]
    ),
    .target(
      name: "TinysHost",
      dependencies: ["TinysCore", "TinysCoreBinding", "TinysDocument", "TinysWidgets"],
      linkerSettings: [.linkedFramework("AppKit"), .linkedFramework("WebKit")]
    ),
    .target(name: "TinysFeatures", dependencies: ["TinysCore"]),
    .target(name: "TinysWidgets"),
    .testTarget(name: "TinysFeaturesTests", dependencies: ["TinysFeatures"]),
    .target(
      name: "TinysCatalog",
      dependencies: [
        "TinysHost", "TinysCore", "TinysDocument", "TinysFeatures",
      ],
      resources: [.process("Resources")],
      linkerSettings: [.linkedFramework("AppKit")]
    ),
    .testTarget(
      name: "TinysCoreTests",
      dependencies: ["TinysCore", "TinysCoreBinding", "TinysDocument", "TinysTestSupport"]),
    .testTarget(
      name: "TinysHostTests",
      dependencies: [
        "TinysHost", "TinysCore", "TinysCoreBinding", "TinysDocument", "TinysTestSupport", "TinysWidgets",
      ]),
    .testTarget(
      name: "TinysCatalogTests",
      dependencies: ["TinysCatalog", "TinysCore", "TinysDocument", "TinysFeatures", "TinysTestSupport"]
    ),
  ],
  swiftLanguageModes: [.v6]
)
