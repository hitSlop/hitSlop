// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "PopApple",
  platforms: [.macOS(.v15), .iOS("26.0")],
  products: [
    .library(name: "PopCore", targets: ["PopCore"]),
    .library(name: "PopFirebase", targets: ["PopFirebase"]),
    .library(name: "PopAccount", targets: ["PopAccount"]),
    .library(name: "PopDocument", targets: ["PopDocument"]),
    .library(name: "PopHost", targets: ["PopHost"]),
    .library(name: "PopCatalog", targets: ["PopCatalog"]),
    .library(name: "PopWidgets", targets: ["PopWidgets"]),
    .library(name: "PopLaunch", targets: ["PopLaunch"]),
  ],
  dependencies: [
    .package(url: "https://github.com/firebase/firebase-ios-sdk.git", exact: "12.19.2"),
    .package(url: "https://github.com/rive-app/rive-ios", exact: "6.29.0"),
  ],
  targets: [
    .binaryTarget(name: "PopCoreFFI", path: "Generated/PopCoreFFI.xcframework"),
    // UniFFI's generated callback vtables predate Swift 6 concurrency checking. The core's
    // store links the platform SQLite.
    .target(
      name: "PopCoreBinding", dependencies: ["PopCoreFFI"], path: "Generated/PopCoreBinding",
      swiftSettings: [.swiftLanguageMode(.v5)], linkerSettings: [.linkedLibrary("sqlite3")]),
    .target(
      name: "PopDocument", dependencies: ["PopCore", "PopCoreBinding", "PopAccount", "PopWidgets"],
      resources: [.copy("Resources/shell")], linkerSettings: [.linkedFramework("WebKit")]),
    .target(
      name: "PopTestSupport", dependencies: ["PopCore", "PopCoreBinding", "PopDocument"],
      path: "Tests/PopTestSupport"),
    .testTarget(
      name: "PopDocumentTests", dependencies: ["PopDocument", "PopCoreBinding", "PopTestSupport"]),
    .target(
      name: "PopCore",
      // The core parses window silhouettes, so manifest geometry has one parser.
      dependencies: ["PopCoreBinding"],
      resources: [.process("Resources")],
      linkerSettings: [.linkedFramework("ImageIO")]
    ),
    .target(
      name: "PopAccount",
      dependencies: [
        "PopCore", "PopCoreBinding",
        .product(name: "FirebaseCore", package: "firebase-ios-sdk"),
        .product(name: "FirebaseAuth", package: "firebase-ios-sdk"),
        .product(name: "FirebaseFirestore", package: "firebase-ios-sdk"),
      ]
    ),
    .target(
      name: "PopFirebase",
      dependencies: [
        "PopCore",
        .product(name: "FirebaseCore", package: "firebase-ios-sdk"),
        .product(name: "FirebaseAnalytics", package: "firebase-ios-sdk"),
        .product(name: "FirebaseCrashlytics", package: "firebase-ios-sdk"),
      ]
    ),
    .target(
      name: "PopHost",
      dependencies: ["PopCore", "PopCoreBinding", "PopDocument", "PopWidgets"],
      linkerSettings: [.linkedFramework("AppKit"), .linkedFramework("WebKit")]
    ),
    .target(name: "PopFeatures", dependencies: ["PopCore"]),
    .target(name: "PopWidgets"),
    .target(
      name: "PopLaunch",
      dependencies: ["PopCore", "PopDocument", .product(name: "RiveRuntime", package: "rive-ios")],
      resources: [.process("Resources")]),
    .testTarget(name: "PopFeaturesTests", dependencies: ["PopFeatures"]),
    .target(
      name: "PopCatalog",
      dependencies: [
        "PopHost", "PopCore", "PopDocument", "PopFeatures", "PopLaunch",
      ],
      resources: [.process("Resources")],
      linkerSettings: [.linkedFramework("AppKit")]
    ),
    .testTarget(
      name: "PopCoreTests",
      dependencies: ["PopCore", "PopCoreBinding", "PopDocument", "PopTestSupport"]),
    .testTarget(
      name: "PopHostTests",
      dependencies: [
        "PopHost", "PopCore", "PopCoreBinding", "PopDocument", "PopTestSupport", "PopWidgets",
      ]),
    .testTarget(
      name: "PopCatalogTests",
      dependencies: ["PopCatalog", "PopCore", "PopDocument", "PopFeatures", "PopTestSupport"]
    ),
  ],
  swiftLanguageModes: [.v6]
)
