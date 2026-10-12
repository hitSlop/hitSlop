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

  // Compile the production resolver into a child so the launcher can set argv[0]
  // independently of the executable, without opening an app through Launch Services.
  let main = folder.appendingPathComponent("main.swift")
  try Data(
    """
    import Foundation
    let registered = URL(fileURLWithPath: CommandLine.arguments[1])
    print(SlopApplicationLocation.application(registered: { registered })!.path)
    """.utf8
  ).write(to: main)
  let source = Fixtures.repository.appendingPathComponent(
    "apps/apple/Packages/HitSlopApple/Sources/HitSlopCore/SlopApplicationLocation.swift")
  let (buildStatus, _, buildErrors) = try Fixtures.run(
    URL(fileURLWithPath: "/usr/bin/xcrun"), ["swiftc", source.path, main.path, "-o", helper.path])
  try #require(buildStatus == 0, "\(buildErrors)")
  for executable in [helper, link] {
    let (status, output, errors) = try Fixtures.run(
      URL(fileURLWithPath: "/bin/bash"),
      [
        "-c", "cd \"$1\" && exec -a hitslop-native \"$2\" \"$3\"", "launcher", folder.path,
        executable.path, oldApp.path,
      ])
    #expect(status == 0, "\(errors)")
    #expect(output.trimmingCharacters(in: .whitespacesAndNewlines) == app.path)
  }
}
