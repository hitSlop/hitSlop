import CoreGraphics
import Foundation
import ImageIO
import SQLite3
import Testing
@testable import HitSlopCore

@Test func rejectsTraversal() { #expect(!SlopPackage.isSafeRelativePath("../data.json")); #expect(SlopPackage.isSafeRelativePath("assets/theme.css")) }

@Test func damagedOptionalGuidanceDoesNotBlockOpening() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let skill = root.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
    try FileManager.default.removeItem(at: skill)
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    try Data("guidance".utf8).write(to: skill)
    let references = skill.deletingLastPathComponent().appendingPathComponent("references")
    try FileManager.default.createDirectory(at: references, withIntermediateDirectories: true)
    try Data([0xff]).write(to: references.appendingPathComponent("app-guide.md"))
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate(requirePreview: false) }
}

@Test func validatesSchemaAndRuntimeBoundary() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    try Data("{}".utf8).write(to: root.appendingPathComponent("package.json"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func allowsHostFinderIconOnlyInDocuments() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try Data().write(to: root.appendingPathComponent("Icon\r"))
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate(requirePreview: false) }
    try Data().write(to: root.appendingPathComponent("other-icon"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func duplicateKeepsManifestIdenticalAndCreatesWritableState() throws {
    let source = try fixture(), temporary = source.deletingLastPathComponent(); defer { try? FileManager.default.removeItem(at: temporary) }
    try extendManifest(source)
    try SlopDuplicator.makeImmutable(source)
    let destination = temporary.appendingPathComponent("copy.slop")
    let openedURL = try SlopDuplicator.duplicate(from: source, to: destination)
    // Recents canonicalizes an existing directory. Duplicate must return that same
    // identity, or reopening its live window attempts a second writer.
    #expect(openedURL == destination.standardizedFileURL.resolvingSymlinksInPath())
    #expect(try Data(contentsOf: source.appendingPathComponent("manifest.json")) == Data(contentsOf: destination.appendingPathComponent("manifest.json")))
    #expect(try SlopPackage(rootURL: destination).manifest.categories == [.productivity, .other])
    #expect(!FileManager.default.fileExists(atPath: destination.appendingPathComponent("stores").path))
    let sourceMode = try FileManager.default.attributesOfItem(atPath: source.path)[.posixPermissions] as? NSNumber
    let destinationMode = try FileManager.default.attributesOfItem(atPath: destination.path)[.posixPermissions] as? NSNumber
    #expect((sourceMode?.intValue ?? 0) & 0o222 == 0)
    #expect((destinationMode?.intValue ?? 0) & 0o200 != 0)
    try FileManager.default.createDirectory(at: destination.appendingPathComponent("state"), withIntermediateDirectories: true)
    try Data(#"{}"#.utf8).write(to: destination.appendingPathComponent("state/theme.json"))
}

@Test func manifestReadsPreserveUnknownMetadataAndRejectMalformedKnownFields() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try extendManifest(root)
    let url = root.appendingPathComponent("manifest.json")
    let original = try Data(contentsOf: url)
    let package = try SlopPackage(rootURL: root)
    #expect(package.manifest.categories == [.productivity, .other])
    #expect(package.shape == .rounded)
    #expect(!package.usesTransparentBackground)
    #expect(package.manifestData == original)
    var manifest = try #require(JSONSerialization.jsonObject(with: original) as? [String: Any])
    for categories in [["future-a", "future-b"], ["other", "future"]] {
        manifest["categories"] = categories
        try JSONSerialization.data(withJSONObject: manifest).write(to: url)
        #expect(try SlopPackage(rootURL: root).manifest.categories == [.other])
    }
    manifest["categories"] = ["utilities"]
    try writeSkin(to: root.appendingPathComponent("assets/skin.png"), width: 320, height: 240)
    manifest["presentation"] = ["width": 320, "height": 240, "skin": "assets/skin.png", "future": true]
    try JSONSerialization.data(withJSONObject: manifest).write(to: url)
    #expect(try SlopPackage(rootURL: root).isSkinned)
    for presentation: [String: Any] in [
        ["width": 1, "height": 1],
        ["width": 320, "height": 240, "skin": "../evil.png"],
        ["width": 320, "height": 240, "skin": NSNull()],
        ["width": 320, "height": 240, "shape": NSNull()],
        ["width": 320, "height": 240, "skin": "assets/skin.png", "resizable": "wrong"],
        ["width": 320, "height": 240, "skin": "assets/skin.png", "resizable": true],
        ["width": 320, "height": 240, "skin": "assets/skin.png", "shape": 42],
        ["width": 320, "height": 240, "skin": "assets/skin.png", "shape": "rounded"],
        ["width": 320, "height": 240, "skin": "assets/skin.png", "background": NSNull()],
    ] {
        manifest["presentation"] = presentation
        #expect(!PlatformContract.valid(manifest, against: manifestReadSchema))
        try JSONSerialization.data(withJSONObject: manifest).write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
    manifest["presentation"] = ["width": 320, "height": 240]
    manifest["categories"] = [String(repeating: "x", count: 65)]
    try JSONSerialization.data(withJSONObject: manifest).write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

private func extendManifest(_ root: URL) throws {
    let url = root.appendingPathComponent("manifest.json")
    var object = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    object["lineage"] = ["template": "future"]
    object["author"] = ["name": "Fixture Author", "handle": "future"]
    object["categories"] = ["productivity", "future-category"]
    object["presentation"] = ["width": 320, "height": 240, "shape": "future-shape", "background": "future-background", "future": true]
    try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys]).write(to: url)
}

// Package validation precedes copying; the SQLite open must also reject a source
// swapped to a symlink after validation, while still copying ordinary databases.
@Test func databaseBackupRejectsSymlinkSource() throws {
    let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: folder) }
    let state = folder.appendingPathComponent("state")
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    let source = folder.appendingPathComponent("source.sqlite")
    let link = state.appendingPathComponent("document.sqlite")
    let destination = folder.appendingPathComponent("copy.sqlite")
    var db: OpaquePointer?
    try #require(sqlite3_open(source.path, &db) == SQLITE_OK)
    try #require(sqlite3_exec(db, "CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES('preserved');", nil, nil, nil) == SQLITE_OK)
    sqlite3_close(db)
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: source)
    #expect(throws: SlopPackageError.self) { try SlopDuplicator.backup(folder, to: destination) }
    #expect(!FileManager.default.fileExists(atPath: destination.path))
    try FileManager.default.removeItem(at: link)
    try FileManager.default.moveItem(at: source, to: link)
    // Also reject a swapped state directory without following it during canonicalization.
    let savedState = folder.appendingPathComponent("saved-state")
    try FileManager.default.moveItem(at: state, to: savedState)
    try FileManager.default.createSymbolicLink(at: state, withDestinationURL: savedState)
    #expect(throws: SlopPackageError.self) { try SlopDuplicator.backup(folder, to: destination) }
    #expect(!FileManager.default.fileExists(atPath: destination.path))
    try FileManager.default.removeItem(at: state)
    try FileManager.default.moveItem(at: savedState, to: state)
    try SlopDuplicator.backup(folder, to: destination)
    try #require(sqlite3_open_v2(destination.path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK)
    defer { sqlite3_close(db) }
    var statement: OpaquePointer?
    try #require(sqlite3_prepare_v2(db, "SELECT value FROM marker", -1, &statement, nil) == SQLITE_OK)
    defer { sqlite3_finalize(statement) }
    try #require(sqlite3_step(statement) == SQLITE_ROW)
    #expect(String(cString: sqlite3_column_text(statement, 0)) == "preserved")
}

@Test func rejectsInvalidSchemaMetadataAndMutableThemeStores() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try Data("not json".utf8).write(to: root.appendingPathComponent("data.schema.json"))
    #expect(throws: (any Error).self) { _ = try SlopPackage(rootURL: root) }
    try FileManager.default.removeItem(at: root.appendingPathComponent("data.schema.json"))
    try FileManager.default.createDirectory(at: root.appendingPathComponent("stores"), withIntermediateDirectories: true)
    try Data([0xff]).write(to: root.appendingPathComponent("stores/theme.css"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func rejectsNoncanonicalSchemaMetadataNames() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try Data(#"{"type":"object"}"#.utf8).write(to: root.appendingPathComponent("schema.json"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func manifestURIFormatDoesNotRepairInvalidAuthorURLs() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    for authorURL in ["https://example.com/with space", "https://example.com/%zz"] {
        manifest["author"] = ["name": "Author", "url": authorURL]
        try JSONSerialization.data(withJSONObject: manifest).write(to: url)
        #expect(throws: (any Error).self) { _ = try SlopPackage(rootURL: root) }
    }
}

@Test func opensChangedEmbeddedDocumentGuidance() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let skill = root.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    try Data("changed".utf8).write(to: skill)
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func opensPackageWithoutDocumentSkill() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try FileManager.default.removeItem(at: root.appendingPathComponent(".agents"))
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func validatesSkinPixelsAndAlpha() throws {
    let root = try fixture(skin: true); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    try writeSkin(to: root.appendingPathComponent("assets/skin.png"), width: 319, height: 240)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func templatesRequireTwoIndependentlyValidStaticPreviews() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let quickLook = root.appendingPathComponent("QuickLook", isDirectory: true)
    try FileManager.default.createDirectory(at: quickLook, withIntermediateDirectories: true)
    let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")!
    try png.write(to: quickLook.appendingPathComponent("Preview.png"))
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate(requirePreview: true) }
    try writeSkin(to: quickLook.appendingPathComponent("Icon.png"), width: 512, height: 512)
    #expect(throws: Never.self) { try SlopPackage(rootURL: root).validateAsTemplate(requirePreview: true) }
    try writeSkin(to: quickLook.appendingPathComponent("Icon.png"), width: 2, height: 1)
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate(requirePreview: true) }
    try Data("different".utf8).write(to: quickLook.appendingPathComponent("Icon.png"))
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate(requirePreview: true) }
}

private func fixture(skin: Bool = false) throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-core-\(UUID().uuidString)", isDirectory: true)
    let root = directory.appendingPathComponent("tiny-counter.slop", isDirectory: true)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    try Data(#"{"format":1,"root":{"kind":"object","properties":{}}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
    let presentation = skin ? #"{"width":320,"height":240,"skin":"assets/skin.png"}"# : #"{"width":320,"height":240}"#
    let manifest = #"{"$schema":"https://api.hitslop.com/schemas/v1/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"tiny-counter","title":"Tiny Counter","description":"Counts things.","categories":["utilities"],"presentation":\#(presentation)}"#
    try Data(manifest.utf8).write(to: root.appendingPathComponent("manifest.json"))
    try writeCanonicalDocumentSkill(to: root)
    if skin { try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true); try writeSkin(to: root.appendingPathComponent("assets/skin.png"), width: 320, height: 240) }
    return root
}

private func writeCanonicalDocumentSkill(to root: URL) throws {
    let skill = root.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
    try FileManager.default.createDirectory(at: skill.deletingLastPathComponent(), withIntermediateDirectories: true)
    try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../../../packages/cli/skills/hitslop-document/SKILL.md").standardizedFileURL).write(to: skill)
}

private func writeSkin(to url: URL, width: Int, height: Int) throws {
    let bytes = Data(repeating: 255, count: width * height * 4)
    guard let provider = CGDataProvider(data: bytes as CFData),
          let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
          let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) else { throw SlopPackageError.invalid("could not create test skin") }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { throw SlopPackageError.invalid("could not write test skin") }
}
