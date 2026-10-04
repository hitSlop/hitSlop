import Foundation
import Testing
@testable import HitSlopCore

@Test func iCloudLocationsRejectNewDestinationsAndAncestorAliases() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("local-location-\(UUID())")
    let cloud = root.appendingPathComponent("cloud")
    try FileManager.default.createDirectory(at: cloud, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let alias = root.appendingPathComponent("alias")
    try FileManager.default.createSymbolicLink(at: alias, withDestinationURL: cloud)
    for path in [cloud, cloud.appendingPathComponent("new/document.slop"), alias.appendingPathComponent("document.slop")] {
        #expect(throws: SlopError.self) { try SlopLocalDocument.requireLocal(path, iCloudRoot: cloud) }
    }
    #expect(throws: Never.self) { try SlopLocalDocument.requireLocal(root.appendingPathComponent("cloud-backup/document.slop"), iCloudRoot: cloud) }
    #expect(!FileManager.default.fileExists(atPath: cloud.appendingPathComponent("new").path))
}
