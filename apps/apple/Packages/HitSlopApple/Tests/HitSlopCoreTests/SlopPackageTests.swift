import CoreGraphics
import Foundation
import ImageIO
import Testing
@testable import HitSlopCore
@testable import HitSlopDocument

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
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate() }
}

@Test func validatesSchemaAndRuntimeBoundary() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    try Data("{}".utf8).write(to: root.appendingPathComponent("package.json"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

// A saved database may contain free pages beyond the authored-asset limit. The
// package walker must identify state correctly even across /var's filesystem alias.
@Test func largeSavedDatabaseIsNotCountedAsAnAuthoredAsset() async throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let owner = try DocumentOwner(package: SlopPackage(rootURL: root))
    let before = try await owner.state()
    try await owner.close()
    let file = try FileHandle(forWritingTo: root.appendingPathComponent("state/document.sqlite"))
    try file.truncate(atOffset: UInt64(SlopFile.maximumBytes + 4096))
    try file.close()
    let reopened = try DocumentOwner(package: SlopPackage(rootURL: root))
    #expect(try await reopened.state() == before)
    try await reopened.close()
    // The exception must remain confined to state, not enlarge the asset allowance.
    let asset = root.appendingPathComponent("assets/oversized.bin")
    try Data().write(to: asset)
    let authored = try FileHandle(forWritingTo: asset)
    try authored.truncate(atOffset: UInt64(SlopFile.maximumBytes + 4096))
    try authored.close()
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func allowsHostFinderIconOnlyInDocuments() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try Data().write(to: root.appendingPathComponent("Icon\r"))
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    #expect(throws: SlopPackageError.self) { try SlopPackage(rootURL: root).validateAsTemplate() }
    try Data().write(to: root.appendingPathComponent("other-icon"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func duplicateKeepsManifestIdenticalAndCreatesWritableState() throws {
    let source = try fixture(), temporary = source.deletingLastPathComponent(); defer { try? FileManager.default.removeItem(at: temporary) }
    try extendManifest(source)
    try SlopPermissions.makeImmutable(source)
    let destination = temporary.appendingPathComponent("copy.slop")
    let openedURL = try SlopDuplicator.duplicate(from: source, to: destination).rootURL
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
    // Theme overrides live in the database; a stray state file is refused.
    try FileManager.default.createDirectory(at: destination.appendingPathComponent("state"), withIntermediateDirectories: true)
    try Data(#"{}"#.utf8).write(to: destination.appendingPathComponent("state/theme.json"))
    #expect(throws: (any Error).self) { _ = try SlopPackage(rootURL: destination) }
}

// Nothing has shipped: a manifest is valid exactly when it matches the contract. Unknown
// fields and values are refused like malformed known ones.
@Test func manifestsAreStrict() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    try extendManifest(root)
    let url = root.appendingPathComponent("manifest.json")
    let original = try Data(contentsOf: url)
    let package = try SlopPackage(rootURL: root)
    #expect(package.manifest.categories == [.productivity, .other])
    #expect(package.silhouette.path(in: CGRect(x: 0, y: 0, width: 320, height: 240)).contains(CGPoint(x: 160, y: 120)))
    #expect(!package.usesTransparentBackground)
    var manifest = try #require(JSONSerialization.jsonObject(with: original) as? [String: Any])
    for (key, value): (String, Any) in [
        ("lineage", ["template": "future"]),
        ("author", ["name": "Fixture Author", "handle": "future"]),
        ("categories", ["productivity", "future-category"]),
        ("presentation", ["width": 320, "height": 240, "future": true]),
        ("presentation", ["width": 320, "height": 240, "background": "future-background"]),
    ] {
        var changed = manifest
        changed[key] = value
        try JSONSerialization.data(withJSONObject: changed).write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
    manifest["categories"] = ["utilities"]
    try writeSkin(to: root.appendingPathComponent("assets/skin.png"), width: 320, height: 240)
    manifest["presentation"] = ["width": 320, "height": 240, "skin": "assets/skin.png"]
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
        ["width": 320, "height": 240, "skin": "assets/skin.png", "shape": "22px"],
        ["width": 320, "height": 240, "skin": "assets/skin.png", "background": NSNull()],
    ] {
        manifest["presentation"] = presentation
        try JSONSerialization.data(withJSONObject: manifest).write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
    manifest["presentation"] = ["width": 320, "height": 240]
    manifest["categories"] = [String(repeating: "x", count: 65)]
    try JSONSerialization.data(withJSONObject: manifest).write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

/// Every optional field the contract allows, with non-default values.
private func extendManifest(_ root: URL) throws {
    let url = root.appendingPathComponent("manifest.json")
    var object = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    object["author"] = ["name": "Fixture Author", "url": "https://example.com"]
    object["categories"] = ["productivity", "other"]
    object["presentation"] = ["width": 320, "height": 240, "shape": "12px 30% / 20px", "resizable": true, "lockAspect": true]
    try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys]).write(to: url)
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

// Failure: a package without theme defaults opened, and its theme commands then failed.
@Test func packagesRequireThemeDefaults() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    _ = try SlopPackage(rootURL: root)
    try FileManager.default.removeItem(at: root.appendingPathComponent("assets/theme.json"))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

private func fixture(skin: Bool = false) throws -> URL {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-core-\(UUID().uuidString)", isDirectory: true)
    let root = directory.appendingPathComponent("tiny-counter.slop", isDirectory: true)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: root.appendingPathComponent("assets/app.js"))
    try Data(#"{"kind":"object","properties":{}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
    try Data("{}".utf8).write(to: root.appendingPathComponent("assets/theme.json"))
    let presentation = skin ? #"{"width":320,"height":240,"skin":"assets/skin.png"}"# : #"{"width":320,"height":240}"#
    let manifest = #"{"$schema":"https://api.hitslop.com/schemas/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"tiny-counter","title":"Tiny Counter","description":"Counts things.","categories":["utilities"],"presentation":\#(presentation)}"#
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

// Shared with the baseline: these acceptance regressions must fail before extraction.
@Test func nativeManifestParityRegressions() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    let original = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    for count in [40, 41, 80, 81] {
        var value = original
        value["title"] = String(repeating: "😀", count: count)
        try JSONSerialization.data(withJSONObject: value).write(to: url)
        let accepted = (try? SlopPackage(rootURL: root)) != nil
        #expect(accepted == (count <= 80), "title with \(count) emoji")
    }
    var value = original
    value["author"] = ["name": "\u{0085}"]
    try JSONSerialization.data(withJSONObject: value).write(to: url)
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    value["author"] = ["name": "Author", "url": "https://[bad]"]
    try JSONSerialization.data(withJSONObject: value).write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    value = original
    value["presentation"] = ["width": 320, "height": 240, "skin": "assets/skin.png\n"]
    try writeSkin(to: root.appendingPathComponent("assets/skin.png\n"), width: 320, height: 240)
    try JSONSerialization.data(withJSONObject: value).write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func manifestInputBoundaryRemainsStrict() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    let original = try Data(contentsOf: url)
    for invalid in [Data([0xff]), Data("{".utf8)] {
        try invalid.write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
    var boundary = original
    boundary.append(Data(repeating: 32, count: 65536 - original.count))
    try boundary.write(to: url)
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    boundary.append(32)
    try boundary.write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func invalidShapesAreRefusedAsInvalidPackages() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    for shape: Any in ["1em", ["path": "M0 0L1"]] {
        manifest["presentation"] = ["width": 320, "height": 240, "shape": shape]
        try JSONSerialization.data(withJSONObject: manifest).write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
}
