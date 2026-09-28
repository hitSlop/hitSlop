import Foundation
import HitSlopCore

/// The same catalog is used by visible sessions, recovery and the installed helper.
struct RuntimeCatalog {
  let root: URL
  let identities: [[String: Any]]

  /// Bundled runtimes are immutable for the life of the process.
  nonisolated(unsafe) private static let cached = Result { () throws -> RuntimeCatalog in
    guard let root = Bundle.module.url(forResource: "runtimes", withExtension: nil) else {
      throw failure("Missing bundled runtimes")
    }
    return try RuntimeCatalog(root: root)
  }

  static func bundled() throws -> RuntimeCatalog { try cached.get() }

  init(root: URL) throws {
    self.root = root
    var entries: [[String: Any]] = []
    for directory in try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil) {
      guard let contract = Int(directory.lastPathComponent), contract > 0,
        String(contract) == directory.lastPathComponent else {
        throw failure("Invalid bundled runtime directory")
      }
      let value = try JSONSerialization.jsonObject(with: SlopFile.read(
        directory.appendingPathComponent("identity.json"), within: root, maximumBytes: 4096))
      guard PlatformContract.valid(value, against: runtimeIdentitySchema),
        let identity = value as? [String: Any], identity["runtimeContract"] as? Int == contract else {
        throw failure("Invalid identity for runtime contract \(contract)")
      }
      for name in ["index.js", "boot.js", "headless.js", "loro/index.js", "loro/loro_wasm_bg.wasm"] {
        let file = directory.appendingPathComponent(name)
        let attributes = try file.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
        guard attributes.isRegularFile == true, attributes.isSymbolicLink != true else {
          throw failure("Incomplete runtime contract \(contract)")
        }
      }
      entries.append(identity)
    }
    guard !entries.isEmpty else { throw failure("No bundled runtime contracts") }
    identities = entries.sorted { ($0["runtimeContract"] as! Int) < ($1["runtimeContract"] as! Int) }
  }

  /// The newest bundled contract, used where no package selects one.
  var currentRuntime: URL {
    root.appendingPathComponent(String(identities.last!["runtimeContract"] as! Int), isDirectory: true)
  }

  func capabilitiesData() throws -> Data {
    try JSONSerialization.data(withJSONObject: ["current": identities.last!, "runtimes": identities], options: [.sortedKeys])
  }

  func resolve(package: SlopPackage) throws -> URL {
    let url = package.rootURL.appendingPathComponent("assets/runtime.json")
    guard FileManager.default.fileExists(atPath: url.path) else {
      throw failure("Missing runtime requirements. Rebuild this template with a current hitSlop CLI.")
    }
    let value: Any
    do {
      value = try JSONSerialization.jsonObject(with: SlopFile.read(url, within: package.rootURL, maximumBytes: 4096))
    } catch { throw failure("Invalid runtime requirements: \(error.localizedDescription)") }
    guard PlatformContract.valid(value, against: runtimeRequirementsSchema),
      let requirements = value as? [String: Any],
      let contract = requirements["runtimeContract"] as? Int,
      let revision = requirements["minRuntimeRevision"] as? Int else {
      throw failure("Invalid runtime requirements. Rebuild this template with a current hitSlop CLI.")
    }
    guard let installed = identities.first(where: { $0["runtimeContract"] as? Int == contract }) else {
      let supported = identities.map { String($0["runtimeContract"] as! Int) }.joined(separator: ", ")
      throw SlopDiagnosticError(failure("This slop requires runtime contract \(contract); this app supports \(supported). Update hitSlop.app."), diagnostic: .init(.rejection, reason: .unsupportedRuntime))
    }
    guard (installed["runtimeRevision"] as! Int) >= revision else {
      throw SlopDiagnosticError(failure("This slop requires runtime contract \(contract), revision \(revision) or newer. Update hitSlop.app."), diagnostic: .init(.rejection, reason: .unsupportedRuntime))
    }
    return root.appendingPathComponent(String(contract), isDirectory: true)
  }
}
