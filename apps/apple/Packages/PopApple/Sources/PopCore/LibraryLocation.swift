@preconcurrency import CloudKit
import Foundation

/// The local index and CloudKit zone must describe the same library scope.
struct LibraryLocation {
  let root: URL
  let zone: CKRecordZone.ID

  init(base: URL, account: String, development: Bool, diagnosticNamespace: String? = nil) throws {
    // The legacy Library zone and index contain unsupported .slop documents. Keep
    // them intact; dotpop's first public lineage starts in its own matching scope.
    var root = base.appendingPathComponent(account, isDirectory: true)
      .appendingPathComponent("PopLibrary", isDirectory: true)
    var zoneName = "PopLibrary"
    if development {
      root = root.appendingPathComponent("Development", isDirectory: true)
      if let namespace = diagnosticNamespace {
        guard !namespace.isEmpty, namespace.utf8.count <= 64,
          namespace.utf8.allSatisfy({
            (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || $0 == 45
          })
        else { throw CloudFailure("Invalid diagnostic library namespace") }
        root = root.appendingPathComponent("Diagnostics/\(namespace)", isDirectory: true)
        zoneName += "-Diagnostic-" + namespace
      }
    }
    self.root = root
    self.zone = CKRecordZone.ID(zoneName: zoneName, ownerName: CKCurrentUserDefaultName)
  }
}
