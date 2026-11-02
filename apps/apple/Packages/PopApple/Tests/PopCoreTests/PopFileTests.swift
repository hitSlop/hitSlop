import CoreGraphics
import Foundation
import ImageIO
import PopCoreBinding
import PopTestSupport
import Testing

@testable import PopCore
@testable import PopDocument

@Test func localSaveFoldersRememberUsableLocationsAndRejectCloudPaths() throws {
  let home = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: home) }
  let suite = "local-save-\(UUID().uuidString)"
  let defaults = try #require(UserDefaults(suiteName: suite))
  defer { defaults.removePersistentDomain(forName: suite) }
  let local = try PopSaveLocation.folder(home: home, defaults: defaults)
  #expect(local == home.appendingPathComponent("pop", isDirectory: true))
  let desktop = home.appendingPathComponent("Desktop", isDirectory: true)
  try FileManager.default.createDirectory(at: desktop, withIntermediateDirectories: true)
  #expect(try PopSaveLocation.folder(home: home, defaults: defaults) == desktop)
  PopSaveLocation.remember(local.appendingPathComponent("document.pop"), defaults: defaults)
  #expect(try PopSaveLocation.folder(home: home, defaults: defaults) == local)
  let cloud = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Mobile Documents")
  #expect(throws: PopFailure.self) {
    try PopLocalDocument.requireLocal(cloud.appendingPathComponent("missing/document.pop"))
  }
  PopSaveLocation.remember(cloud.appendingPathComponent("document.pop"), defaults: defaults)
  #expect(try PopSaveLocation.folder(home: home, defaults: defaults) == local)
  try FileManager.default.removeItem(at: local)
  #expect(try PopSaveLocation.folder(home: home, defaults: defaults) == desktop)
}

// The core owns the file's rules (crates/pop-core/tests/file.rs and manifest.rs).
// These prove Swift reads what the core checked: the manifest, the window, the theme's
// order and the file's kind, and that it maps the core's refusals.

@Test func aPopIsOneRegularFileNeverALinkOrAFolder() throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let document = try Fixtures.document(stage: stage(), at: folder.appendingPathComponent("document.pop"))
  #expect(throws: Never.self) { _ = try PopFile(url: document) }
  let link = folder.appendingPathComponent("link.pop")
  try FileManager.default.createSymbolicLink(at: link, withDestinationURL: document)
  #expect(throws: PopError.self) { _ = try PopFile(url: link) }
  let directory = folder.appendingPathComponent("folder.pop")
  try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  #expect(throws: PopError.self) { _ = try PopFile(url: directory) }
  let renamed = folder.appendingPathComponent("renamed.sqlite")
  try FileManager.default.copyItem(at: document, to: renamed)
  #expect(throws: PopError.self) { _ = try PopFile(url: renamed) }
  let foreign = folder.appendingPathComponent("foreign.pop")
  try Fixtures.sql(foreign, "CREATE TABLE t(x)")
  #expect(throws: PopError.self) { _ = try PopFile(url: foreign) }
}

@Test func readsTheCheckedAppOfTemplatesAndDocuments() throws {
  // Every optional field the contract allows, with non-default values, and a theme in its
  // author's order.
  let source = try Fixtures.minimalStage(
    slug: "pop-counter",
    fields: [
      "author": ["name": "Fixture Author", "url": "https://example.com"],
      "categories": ["productivity", "other"],
      "window": ["width": 320, "height": 240, "shape": "12px 30% / 20px", "resizable": true, "lockAspect": true],
    ], theme: [("paper", "#ffffff"), ("accent", "#335577")])
  let template = try Fixtures.template(stage: source)
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let document = template.deletingLastPathComponent().appendingPathComponent("document.pop")
  _ = try PopFile.create(from: template, to: document)
  for (url, kind) in [(template, FileKind.template), (document, .document)] {
    let file = try PopFile(url: url)
    #expect(file.kind == kind)
    #expect(file.packageFormat == PackageFormat.level)
    #expect(file.runtimeABI == RuntimeABI.level)
    #expect(file.metadata.categories == [.productivity, .other])
    guard case .shape(let silhouette) = file.outline else {
      Issue.record("Expected a shape")
      return
    }
    #expect(silhouette.path(in: CGRect(x: 0, y: 0, width: 320, height: 240)).contains(CGPoint(x: 160, y: 120)))
    #expect(file.backdrop == .window && file.isResizable)
    // The panel lists colors in the order the author declared them.
    #expect(file.themeTokens.map(\.name) == ["paper", "accent"])
    #expect(file.byteCount == (try url.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init))
  }
}

@Test func decodesTheWindowSkin() throws {
  let source = try stage(skin: true)
  let document = try Fixtures.document(stage: source)
  defer { try? FileManager.default.removeItem(at: document) }
  let file = try PopFile(url: document)
  guard case .skin(let image) = file.outline else {
    Issue.record("Expected a skin")
    return
  }
  #expect(file.backdrop == .skin && !file.isResizable && file.lockAspect && file.fitsFullscreen)
  #expect(image.width == 320 && image.height == 240)
}

@Test func aSkinFrameRequiresItsDecodedImage() throws {
  let document = try Fixtures.document(stage: stage(skin: true))
  defer { try? FileManager.default.removeItem(at: document) }
  var opened = try openFile(path: document.path)
  opened.skinPng = nil
  #expect(throws: PopError.self) { _ = try PopFile(url: document, opened: opened) }
  opened.skinPng = Data("invalid PNG".utf8)
  #expect(throws: PopError.self) { _ = try PopFile(url: document, opened: opened) }
}

@Test func theBackdropFollowsThePresentationsBackground() throws {
  for (background, backdrop) in [("transparent", PopBackdrop.clear), ("glass", .glass)] {
    let stage = try Fixtures.minimalStage(
      slug: background, fields: ["window": ["width": 240, "height": 180, "background": background]])
    let document = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: document) }
    #expect(try PopFile(url: document).backdrop == backdrop)
  }
}

// A file a newer pop wrote asks for an update and is never reported as damaged.
@Test func aNewerFileAsksForAnUpdate() throws {
  let document = try Fixtures.document(stage: stage())
  defer { try? FileManager.default.removeItem(at: document) }
  try Fixtures.sql(document, "UPDATE app SET runtime_abi = \(RuntimeABI.level + 1)")
  #expect(throws: PopRequiresUpdate.self) { _ = try PopFile(url: document) }
  #expect(PopFailureContext.classify(PopRequiresUpdate()).reason == .requiresUpdate)
}

@Test func creationReturnsTheCanonicalURLAndNeverReplaces() throws {
  let template = try Fixtures.template(stage: stage())
  let folder = template.deletingLastPathComponent()
  defer { try? FileManager.default.removeItem(at: folder) }
  let created = folder.appendingPathComponent("created.pop")
  // Recents canonicalizes the URL; creation must return that same identity, or reopening
  // its live window attempts a second writer.
  #expect(try PopFile.create(from: template, to: created) == created.standardizedFileURL.resolvingSymlinksInPath())
  let second = folder.appendingPathComponent("second.pop")
  _ = try PopFile.create(from: template, to: second)
  #expect(try PopFile(url: second).metadata.slug == PopFile(url: created).metadata.slug)
  // Creation never replaces an existing file.
  #expect(throws: (any Error).self) { try PopFile.create(from: template, to: second) }
}

/// A small app's build stage, with an optional window skin.
private func stage(skin: Bool = false) throws -> URL {
  let window: [String: Any] =
    ["width": 320, "height": 240]
  let stage = try Fixtures.minimalStage(slug: "pop-counter", fields: ["window": window])
  if skin {
    try writeSkin(to: stage.appendingPathComponent("assets/skin.png"), width: 320, height: 240)
    try Fixtures.addSkin(stage, path: "assets/skin.png")
  }
  return stage
}

private func writeSkin(to url: URL, width: Int, height: Int) throws {
  try Fixtures.png(width: width, height: height) { _, _ in 255 }.write(to: url)
}

@Test func incomingSharingActivityPreservesTheCapabilityAndReportsMissingFragments() throws {
  let link =
    "https://share.dotpop.click/join#" + String(repeating: "a", count: 32) + "." + String(repeating: "b", count: 64)
  let activity = NSUserActivity(activityType: NSUserActivityTypeBrowsingWeb)
  activity.webpageURL = try #require(URL(string: link))
  #expect(try SharingLinks.incomingLink(#require(activity.webpageURL)) == link)
  for value in ["https://share.dotpop.click/join", "https://share.dotpop.click/join#"] {
    do {
      _ = try SharingLinks.incomingLink(#require(URL(string: value)))
      Issue.record("A missing capability must be explained")
    } catch { #expect(error.localizedDescription == "Open the link in pop again") }
  }
  #expect(throws: (any Error).self) {
    _ = try SharingLinks.incomingLink(#require(URL(string: "https://share.dotpop.click/join#invalid")))
  }
  for value in ["https://example.com/join", "https://share.dotpop.click/", "dotpop://widgets"] {
    #expect(try SharingLinks.incomingLink(#require(URL(string: value))) == nil)
  }
}
