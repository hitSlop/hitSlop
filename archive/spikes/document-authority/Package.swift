// swift-tools-version: 6.0
import PackageDescription
let package = Package(
  name: "DocumentAuthoritySpike", platforms: [.macOS(.v15)],
  products: [.executable(name: "document-authority", targets: ["Harness"])],
  targets: [.executableTarget(name: "Harness", linkerSettings: [
    .linkedFramework("JavaScriptCore"), .linkedFramework("AppKit"),
    .linkedFramework("WebKit"), .linkedLibrary("sqlite3"),
  ])], swiftLanguageModes: [.v5]
)
