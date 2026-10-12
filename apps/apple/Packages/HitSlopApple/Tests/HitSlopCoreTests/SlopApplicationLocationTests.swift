import Foundation
import HitSlopTestSupport
import Testing

@testable import HitSlopCore

@Test func bundledHelperOpensItsOwnApplicationInsteadOfAnotherRegisteredCopy() throws {
  let folder = try Fixtures.folder().resolvingSymlinksInPath()
  defer { try? FileManager.default.removeItem(at: folder) }
  let app = folder.appendingPathComponent("Renamed hitSlop.app", isDirectory: true)
  let helpers = app.appendingPathComponent("Contents/Helpers")
  try FileManager.default.createDirectory(at: helpers, withIntermediateDirectories: true)
  let plist = try PropertyListSerialization.data(
    fromPropertyList: ["CFBundleIdentifier": "com.hitslop.app", "CFBundlePackageType": "APPL"],
    format: .xml, options: 0)
  try plist.write(to: app.appendingPathComponent("Contents/Info.plist"))
  let helper = helpers.appendingPathComponent("hitslop-native")
  try Data().write(to: helper)
  let oldApp = folder.appendingPathComponent("Old worktree/hitSlop.app")
  #expect(SlopApplicationLocation.application(for: helper, registered: { oldApp }) == app)
  #expect(SlopApplicationLocation.application(for: helper, registered: { nil }) == app)
  let link = folder.appendingPathComponent("helper-link")
  try FileManager.default.createSymbolicLink(at: link, withDestinationURL: helper)
  #expect(SlopApplicationLocation.application(for: link, registered: { oldApp }) == app)
  let standalone = folder.appendingPathComponent("debug/hitslop-native")
  #expect(SlopApplicationLocation.application(for: standalone, registered: { oldApp }) == oldApp)
  #expect(SlopApplicationLocation.application(for: standalone, registered: { nil }) == nil)
}
