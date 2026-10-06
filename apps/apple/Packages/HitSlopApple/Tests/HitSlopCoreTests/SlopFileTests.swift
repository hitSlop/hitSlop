import CoreGraphics
import Foundation
import HitSlopCoreBinding
import HitSlopTestSupport
import ImageIO
import Testing

@testable import HitSlopCore
@testable import HitSlopDocument

// The core owns the file's rules (crates/hitslop-core/tests/file.rs and manifest.rs).
// These prove Swift reads what the core checked: the manifest, the window, the theme's
// order and the file's kind, and that it maps the core's refusals.

@Test func aSlopIsOneRegularFileNeverALinkOrAFolder() throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let document = try Fixtures.document(stage: stage(), at: folder.appendingPathComponent("document.slop"))
  #expect(throws: Never.self) { _ = try SlopFile(url: document) }
  let link = folder.appendingPathComponent("link.slop")
  try FileManager.default.createSymbolicLink(at: link, withDestinationURL: document)
  #expect(throws: SlopError.self) { _ = try SlopFile(url: link) }
  let directory = folder.appendingPathComponent("folder.slop")
  try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  #expect(throws: SlopError.self) { _ = try SlopFile(url: directory) }
  let renamed = folder.appendingPathComponent("renamed.sqlite")
  try FileManager.default.copyItem(at: document, to: renamed)
  #expect(throws: SlopError.self) { _ = try SlopFile(url: renamed) }
  let foreign = folder.appendingPathComponent("foreign.slop")
  try Fixtures.sql(foreign, "CREATE TABLE t(x)")
  #expect(throws: SlopError.self) { _ = try SlopFile(url: foreign) }
}

@Test func readsTheCheckedAppOfTemplatesAndDocuments() throws {
  // Every optional field the contract allows, with non-default values, and a theme in its
  // author's order.
  let source = try Fixtures.minimalStage(
    slug: "tiny-counter",
    manifest: [
      "author": ["name": "Fixture Author", "url": "https://example.com"],
      "categories": ["productivity", "other"],
      "presentation": ["width": 320, "height": 240, "shape": "12px 30% / 20px", "resizable": true, "lockAspect": true],
    ], theme: ##"{"paper":"#ffffff","accent":"#335577"}"##)
  let template = try Fixtures.template(stage: source)
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let document = template.deletingLastPathComponent().appendingPathComponent("document.slop")
  _ = try SlopFile.create(from: template, to: document)
  for (url, kind) in [(template, FileKind.template), (document, .document)] {
    let file = try SlopFile(url: url)
    #expect(file.kind == kind)
    #expect(file.manifest.categories == [.productivity, .other])
    #expect(file.silhouette.path(in: CGRect(x: 0, y: 0, width: 320, height: 240)).contains(CGPoint(x: 160, y: 120)))
    #expect(file.backdrop == .window && file.isResizable && !file.isSkinned)
    // The panel lists colors in the order the author declared them.
    #expect(file.themeTokens.map(\.name) == ["paper", "accent"])
    #expect(file.byteCount == (try url.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init))
  }
}

@Test func decodesTheWindowSkin() throws {
  let source = try stage(skin: true)
  let document = try Fixtures.document(stage: source)
  defer { try? FileManager.default.removeItem(at: document) }
  let file = try SlopFile(url: document)
  #expect(file.isSkinned && file.backdrop == .skin && !file.isResizable)
  #expect(file.skin?.width == 320 && file.skin?.height == 240)
}

@Test func theBackdropFollowsThePresentationsBackground() throws {
  for (background, backdrop) in [("transparent", SlopBackdrop.clear), ("glass", .glass)] {
    let stage = try Fixtures.minimalStage(
      slug: background, manifest: ["presentation": ["width": 240, "height": 180, "background": background]])
    let document = try Fixtures.document(stage: stage)
    defer { try? FileManager.default.removeItem(at: document) }
    #expect(try SlopFile(url: document).backdrop == backdrop)
  }
}

// A file a newer hitSlop wrote asks for an update and is never reported as damaged.
@Test func aNewerFileAsksForAnUpdate() throws {
  let document = try Fixtures.document(stage: stage())
  defer { try? FileManager.default.removeItem(at: document) }
  try Fixtures.sql(document, "UPDATE app SET runtime_abi = \(RuntimeABI.level + 1)")
  #expect(throws: SlopRequiresUpdate.self) { _ = try SlopFile(url: document) }
  #expect(SlopFailureContext.classify(SlopRequiresUpdate()).reason == .requiresUpdate)
}

@Test func creationReturnsTheCanonicalURLAndNeverReplaces() throws {
  let template = try Fixtures.template(stage: stage())
  let folder = template.deletingLastPathComponent()
  defer { try? FileManager.default.removeItem(at: folder) }
  let created = folder.appendingPathComponent("created.slop")
  // Recents canonicalizes the URL; creation must return that same identity, or reopening
  // its live window attempts a second writer.
  #expect(try SlopFile.create(from: template, to: created) == created.standardizedFileURL.resolvingSymlinksInPath())
  let second = folder.appendingPathComponent("second.slop")
  _ = try SlopFile.create(from: template, to: second)
  #expect(try SlopFile(url: second).manifest.slug == SlopFile(url: created).manifest.slug)
  // Creation never replaces an existing file.
  #expect(throws: (any Error).self) { try SlopFile.create(from: template, to: second) }
}

/// A small app's build stage, with an optional window skin.
private func stage(skin: Bool = false) throws -> URL {
  let presentation: [String: Any] =
    skin ? ["width": 320, "height": 240, "skin": "assets/skin.png"] : ["width": 320, "height": 240]
  let stage = try Fixtures.minimalStage(slug: "tiny-counter", manifest: ["presentation": presentation])
  if skin { try writeSkin(to: stage.appendingPathComponent("assets/skin.png"), width: 320, height: 240) }
  return stage
}

private func writeSkin(to url: URL, width: Int, height: Int) throws {
  try Fixtures.png(width: width, height: height) { _, _ in 255 }.write(to: url)
}
