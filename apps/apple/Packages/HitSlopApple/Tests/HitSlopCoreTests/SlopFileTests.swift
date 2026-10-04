import CoreGraphics
import Foundation
import ImageIO
import SQLite3
import Testing
import HitSlopTestSupport
@testable import HitSlopCore
@testable import HitSlopDocument

// The core owns the file's rules (crates/hitslop-core/tests/file.rs and manifest.rs).
// These prove Swift reads what the core checked: the manifest, the window, the theme's
// order and the file's kind, and that it maps the core's refusals.


@Test func aSlopIsOneRegularFileNeverALinkOrAFolder() throws {
    let folder = try Fixtures.folder(); defer { try? FileManager.default.removeItem(at: folder) }
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
    try sql(foreign, "CREATE TABLE t(x)")
    #expect(throws: SlopError.self) { _ = try SlopFile(url: foreign) }
}

@Test func readsTheCheckedAppOfTemplatesAndDocuments() throws {
    // Every optional field the contract allows, with non-default values, and a theme in its
    // author's order.
    let source = try Fixtures.minimalStage(slug: "tiny-counter", manifest: [
        "author": ["name": "Fixture Author", "url": "https://example.com"],
        "categories": ["productivity", "other"],
        "presentation": ["width": 320, "height": 240, "shape": "12px 30% / 20px", "resizable": true, "lockAspect": true],
    ], theme: ##"{"paper":"#ffffff","accent":"#335577"}"##)
    let template = try Fixtures.template(stage: source)
    defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
    let document = template.deletingLastPathComponent().appendingPathComponent("document.slop")
    try SlopFile.create(from: template, to: document)
    for (url, kind) in [(template, SlopFile.Kind.template), (document, .document)] {
        let file = try SlopFile(url: url)
        #expect(file.kind == kind)
        #expect(file.manifest.categories == [.productivity, .other])
        #expect(file.silhouette.path(in: CGRect(x: 0, y: 0, width: 320, height: 240)).contains(CGPoint(x: 160, y: 120)))
        #expect(!file.usesTransparentBackground && file.isResizable && !file.isSkinned)
        // The panel lists colors in the order the author declared them.
        #expect(file.themeTokens.map(\.name) == ["paper", "accent"])
        #expect(file.runtimeABI == RuntimeABI.level)
        #expect(file.byteCount == (try url.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init))
    }
}

@Test func decodesTheWindowSkin() throws {
    let source = try stage(skin: true)
    let document = try Fixtures.document(stage: source)
    defer { try? FileManager.default.removeItem(at: document) }
    let file = try SlopFile(url: document)
    #expect(file.isSkinned && file.usesTransparentBackground && !file.isResizable)
    #expect(file.skin?.width == 320 && file.skin?.height == 240)
}

// A file a newer hitSlop wrote asks for an update and is never reported as damaged.
@Test func aNewerFileAsksForAnUpdate() throws {
    let document = try Fixtures.document(stage: stage())
    defer { try? FileManager.default.removeItem(at: document) }
    try sql(document, "UPDATE app SET runtime_abi = \(RuntimeABI.level + 1)")
    #expect(throws: SlopRequiresUpdate.self) { _ = try SlopFile(url: document) }
    #expect(SlopFailureContext.classify(SlopRequiresUpdate()).reason == .requiresUpdate)
}

@Test func creationReturnsTheCanonicalURLAndNeverReplaces() throws {
    let template = try Fixtures.template(stage: stage())
    let folder = template.deletingLastPathComponent(); defer { try? FileManager.default.removeItem(at: folder) }
    let created = folder.appendingPathComponent("created.slop")
    // Recents canonicalizes the URL; creation must return that same identity, or reopening
    // its live window attempts a second writer.
    #expect(try SlopFile.create(from: template, to: created) == created.standardizedFileURL.resolvingSymlinksInPath())
    let second = folder.appendingPathComponent("second.slop")
    try SlopFile.create(from: template, to: second)
    #expect(try SlopFile(url: second).manifest.slug == SlopFile(url: created).manifest.slug)
    // Creation never replaces an existing file.
    #expect(throws: (any Error).self) { try SlopFile.create(from: template, to: second) }
}

/// A small app's build stage, with an optional window skin.
private func stage(skin: Bool = false) throws -> URL {
    let presentation: [String: Any] = skin ? ["width": 320, "height": 240, "skin": "assets/skin.png"] : ["width": 320, "height": 240]
    let stage = try Fixtures.minimalStage(slug: "tiny-counter", manifest: ["presentation": presentation])
    if skin { try writeSkin(to: stage.appendingPathComponent("assets/skin.png"), width: 320, height: 240) }
    return stage
}

private func writeSkin(to url: URL, width: Int, height: Int) throws {
    let bytes = Data(repeating: 255, count: width * height * 4)
    guard let provider = CGDataProvider(data: bytes as CFData),
          let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
          let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) else { throw SlopError.invalid("could not create test skin") }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { throw SlopError.invalid("could not write test skin") }
}

/// Runs one statement on `url` outside the core, as a damaged or foreign writer would.
private func sql(_ url: URL, _ statement: String) throws {
    var db: OpaquePointer?
    defer { sqlite3_close(db) }
    guard sqlite3_open(url.path, &db) == SQLITE_OK, sqlite3_exec(db, statement, nil, nil, nil) == SQLITE_OK
    else { throw SlopError.invalid(String(cString: sqlite3_errmsg(db))) }
}
