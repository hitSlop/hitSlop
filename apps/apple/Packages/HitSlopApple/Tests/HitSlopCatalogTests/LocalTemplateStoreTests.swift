import AppKit
import ComposableArchitecture
import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopHost
import HitSlopDocument
import Testing
import SwiftUI
@testable import HitSlopCatalog

// Same-path artwork replacement must update an already mounted catalog, without reselection.
@Test(arguments: [false, true]) @MainActor func displayedCatalogArtworkRefreshesAfterSamePathReplacement(icon: Bool) async throws {
    _ = NSApplication.shared
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try writeTemplate(named: "preview-refresh", in: root)
    let artwork = package.appendingPathComponent(icon ? "QuickLook/Icon.png" : "QuickLook/Preview.png")
    try coloredArtwork(.red).write(to: artwork)
    try FileManager.default.setAttributes([.modificationDate: Date(timeIntervalSince1970: 100)], ofItemAtPath: artwork.path)
    let scanner = CatalogScanner()
    let templates = root.appendingPathComponent("templates")
    var initial = CatalogFeature.State()
    initial.isStarted = true
    initial.filter = .recents
    initial.recents = try await scanner.recents([package], templatesRoot: templates)
    initial.selectedID = try #require(initial.recents.first).id
    let store = Store(initialState: initial) { CatalogFeature() } withDependencies: {
        $0.catalogClient.recents = { (try? await scanner.recents([package], templatesRoot: templates)) ?? [] }
        $0.catalogClient.recent = { try? await scanner.recent($0) }
        $0.catalogClient.refreshLocal = { _ in }
    }
    let host = NSHostingView(rootView: CatalogView(store: store))
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720),
        styleMask: [.titled], backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    window.contentView = host
    window.orderFront(nil)
    defer { window.close() }
    try await expectPreview(in: host, blue: false, minimumPixels: icon ? 30 : 100)
    if icon {
        SlopPreviewWriter.installFinderIcon(try coloredArtwork(.blue), for: package)
    } else {
        try SlopPreviewWriter.write(coloredArtwork(.blue), to: package)
    }
    try await expectPreview(in: host, blue: true, minimumPixels: icon ? 30 : 100)
    #expect(store.selectedID == initial.selectedID)
}

@MainActor private func coloredArtwork(_ color: NSColor) throws -> Data {
    let bitmap = try #require(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 512, pixelsHigh: 512,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    color.setFill()
    NSRect(x: 0, y: 0, width: 512, height: 512).fill()
    NSGraphicsContext.restoreGraphicsState()
    return try #require(bitmap.representation(using: .png, properties: [:]))
}

@MainActor private func expectPreview(in view: NSView, blue: Bool, minimumPixels: Int) async throws {
    let deadline = ContinuousClock.now.advanced(by: .seconds(5))
    repeat {
        view.layoutSubtreeIfNeeded()
        if let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) {
            view.cacheDisplay(in: view.bounds, to: bitmap)
            let png = try #require(bitmap.representation(using: .png, properties: [:]))
            let pixelsImage = try #require(NSBitmapImageRep(data: png))
            var pixels = 0
            for y in stride(from: 0, to: bitmap.pixelsHigh, by: 8) {
                for x in stride(from: 0, to: bitmap.pixelsWide, by: 8) {
                    guard let color = pixelsImage.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
                    if color.greenComponent < 0.5,
                       blue ? color.blueComponent > 0.7 && color.redComponent < 0.3
                            : color.redComponent > 0.7 && color.blueComponent < 0.4 { pixels += 1 }
                }
            }
            if pixels > minimumPixels { return }
        }
        // SwiftUI exposes no completion callback for display; wait for the rendered fixture color.
        try await Task.sleep(for: .milliseconds(20))
    } while ContinuousClock.now < deadline
    Issue.record("Catalog did not display the \(blue ? "replacement blue" : "initial red") artwork")
}

@Test @MainActor func discoversAndDuplicatesInstalledTemplate() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try writeTemplate(named: "tiny-counter", in: root)
    let manifestURL = package.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: manifestURL)) as? [String: Any])
    manifest["categories"] = ["utilities", "other"]
    try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted]).write(to: manifestURL)

    let store = LocalTemplateStore(templatesURL: root)
    await store.refresh()
    #expect(store.templates.count == 1)
    #expect(store.templates.first?.manifest.title == "Tiny Counter")
    #expect(store.templates.first?.icon.url.lastPathComponent == "Icon.png")
    #expect(store.issues.isEmpty)

    let destination = root.appendingPathComponent("created.slop", isDirectory: true)
    let created = try SlopDuplicator.duplicate(from: #require(store.templates.first).packageURL, to: destination, fromTemplate: true)
    SlopPreviewWriter.installAuthoredIcon(for: created)
    #expect(try Data(contentsOf: package.appendingPathComponent("manifest.json")) == Data(contentsOf: destination.appendingPathComponent("manifest.json")))
    #expect(try SlopPackage(rootURL: destination).manifest.categories == [.utilities, .other])
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("assets/app.js").path))
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("QuickLook/Preview.png").path))
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("QuickLook/Icon.png").path))
    let updatedPreview = Data("updated preview".utf8)
    try SlopPreviewWriter.write(updatedPreview, to: destination)
    #expect(try Data(contentsOf: destination.appendingPathComponent("QuickLook/Preview.png")) == updatedPreview)
    #expect(try Data(contentsOf: destination.appendingPathComponent("QuickLook/Icon.png")) == iconPNG)
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("Icon\r").path))
    let updatedIcon = try coloredArtwork(.blue)
    SlopPreviewWriter.installFinderIcon(updatedIcon, for: destination)
    #expect(try Data(contentsOf: destination.appendingPathComponent("QuickLook/Icon.png")) == updatedIcon)
    #expect(try Data(contentsOf: package.appendingPathComponent("QuickLook/Icon.png")) == iconPNG)
    #expect(!FileManager.default.fileExists(atPath: package.appendingPathComponent("Icon\r").path))
    #expect(!FileManager.default.fileExists(atPath: destination.appendingPathComponent("stores").path))
}

@Test @MainActor func failedIconWritePreservesPreviousArtwork() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try writeTemplate(named: "icon-write-failure", in: root)
    SlopPreviewWriter.installAuthoredIcon(for: try SlopPackage(rootURL: package))
    let icon = package.appendingPathComponent("QuickLook/Icon.png")
    let finderIcon = package.appendingPathComponent("Icon\r/..namedfork/rsrc")
    let previousFinderIcon = try Data(contentsOf: finderIcon)
    try FileManager.default.setAttributes([.immutable: true], ofItemAtPath: icon.path)
    defer { try? FileManager.default.setAttributes([.immutable: false], ofItemAtPath: icon.path) }
    var failures: [SlopTelemetryEvent] = []
    SlopPreviewWriter.installFinderIcon(try coloredArtwork(.blue), for: package,
        telemetry: SlopTelemetry { if case .failed = $0 { failures.append($0) } })
    #expect(try Data(contentsOf: icon) == iconPNG)
    #expect(try Data(contentsOf: finderIcon) == previousFinderIcon)
    #expect(failures.count == 1)
}

@Test @MainActor func discoversOnlyTopLevelPackages() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    _ = try writeTemplate(named: "tiny-counter", in: root)
    _ = try writeTemplate(named: "cached-slop", in: root.appendingPathComponent("nested/archived/cached-slop", isDirectory: true), fileName: "1.slop")

    let store = LocalTemplateStore(templatesURL: root)
    await store.refresh()
    #expect(store.templates.map(\.manifest.slug) == ["tiny-counter"])
    #expect(store.issues.isEmpty)
}

private let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")!
private let iconPNG = try! makeIconPNG()

private func writeTemplate(named slug: String, in directory: URL, fileName: String? = nil) throws -> URL {
    let package = directory.appendingPathComponent(fileName ?? "\(slug).slop", isDirectory: true)
    try FileManager.default.createDirectory(at: package.appendingPathComponent("QuickLook"), withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: package.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: package.appendingPathComponent("assets/app.js"))
    try Data(#"{"kind":"object","properties":{}}"#.utf8).write(to: package.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: package.appendingPathComponent("initial.json"))
    try Data("{}".utf8).write(to: package.appendingPathComponent("assets/theme.json"))
    let manifest = #"{"$schema":"https://api.hitslop.com/schemas/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"\#(slug)","title":"Tiny Counter","description":"Counts a very small thing.","categories":["utilities","personal"],"presentation":{"width":320,"height":240}}"#
    try Data(manifest.utf8).write(to: package.appendingPathComponent("manifest.json"))
    let skill = package.appendingPathComponent(".agents/skills/hitslop-document/SKILL.md")
    try FileManager.default.createDirectory(at: skill.deletingLastPathComponent(), withIntermediateDirectories: true)
    try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../../../packages/cli/skills/hitslop-document/SKILL.md").standardizedFileURL).write(to: skill)
    try png.write(to: package.appendingPathComponent("QuickLook/Preview.png"))
    try iconPNG.write(to: package.appendingPathComponent("QuickLook/Icon.png"))
    return package
}

private func makeIconPNG() throws -> Data {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 512, pixelsHigh: 512, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    return bitmap.representation(using: .png, properties: [:])!
}

// Installed templates are validated once per version: an unchanged package is listed
// without being read again, and a replaced one is validated anew.
@Test func templatesAreValidatedOncePerVersion() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try writeTemplate(named: "cached", in: root)
    let scanner = CatalogScanner()
    #expect(try await scanner.local(at: root, makeImmutable: false).templates.count == 1)
    // Editing a file in place leaves the package's version unchanged.
    try Data("not json".utf8).write(to: package.appendingPathComponent("initial.json"))
    #expect(try await scanner.local(at: root, makeImmutable: false).templates.count == 1)
    try FileManager.default.setAttributes([.modificationDate: Date().addingTimeInterval(60)],
        ofItemAtPath: package.appendingPathComponent("manifest.json").path)
    let rescanned = try await scanner.local(at: root, makeImmutable: false)
    #expect(rescanned.templates.isEmpty)
    #expect(rescanned.issues.count == 1)
}

@Test @MainActor func templateIdentityIncludesItsSource() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    for source in ["built-in", "registered"] { _ = try writeTemplate(named: "same-slug", in: root.appendingPathComponent(source)) }
    let scanner = CatalogScanner()
    let builtIn = try await scanner.local(at: root.appendingPathComponent("built-in"), makeImmutable: false)
    let local = try await scanner.local(at: root.appendingPathComponent("registered"))
    let first = CatalogServices.localEntry(try #require(builtIn.templates.first))
    let second = CatalogServices.localEntry(try #require(local.templates.first))
    #expect(first.id != second.id)
}

@Test @MainActor func localCatalogCombinesSourcesAndReportsInvalidPackages() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let bundledRoot = root.appendingPathComponent("bundled"), installedRoot = root.appendingPathComponent("installed")
    _ = try writeTemplate(named: "same-slug", in: bundledRoot)
    _ = try writeTemplate(named: "same-slug", in: installedRoot)
    try FileManager.default.createDirectory(at: installedRoot.appendingPathComponent("invalid.slop"), withIntermediateDirectories: true)
    let services = CatalogServices(templatesURL: installedRoot, bundledRoot: bundledRoot)
    let stream = await services.client.local()
    var iterator = stream.makeAsyncIterator()
    var snapshot = await iterator.next()
    if snapshot?.entries.count == 1 { snapshot = await iterator.next() }
    let entries = try #require(snapshot?.entries)
    #expect(entries.count == 2)
    #expect(entries.map(\.isBundled) == [true, false])
    #expect(Set(entries.map(\.id)).count == 2)
    #expect(entries.allSatisfy { $0.categories == [.utilities, .personal] })
    #expect(snapshot?.issues.count == 1)
}

@Test @MainActor func creationTelemetryExcludesCancellationAndKeepsTheMasterUnchanged() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let templates = root.appendingPathComponent("templates")
    let source = try writeTemplate(named: "tiny-counter", in: templates)
    let before = try Data(contentsOf: source.appendingPathComponent("initial.json"))
    let scanner = CatalogScanner()
    let snapshot = try await scanner.local(at: templates)
    let entry = CatalogServices.localEntry(try #require(snapshot.templates.first))
    var events: [SlopTelemetryEvent] = []
    var recent: URL?
    var destination: URL?
    let services = CatalogServices(templatesURL: templates, bundledRoot: nil,
        telemetry: SlopTelemetry { if case .breadcrumb = $0 { return }; events.append($0) },
        chooseDestination: { _ in destination }, recordRecent: { recent = $0 })
    #expect(try await services.client.chooseDestination(entry) == nil)
    #expect(events.isEmpty && recent == nil)
    destination = root.appendingPathComponent("created.slop")
    let chosen = try #require(try await services.client.chooseDestination(entry))
    #expect(try await services.client.create(entry, chosen) == destination?.standardizedFileURL.resolvingSymlinksInPath())
    #expect(events == [.created(.installed)])
    #expect(recent == destination)
    #expect(try Data(contentsOf: source.appendingPathComponent("initial.json")) == before)
    #expect(!FileManager.default.fileExists(atPath: source.appendingPathComponent("state").path))
    // Existing destinations fail without emitting another creation.
    await #expect(throws: (any Error).self) { _ = try await services.client.create(entry, chosen) }
    #expect(events == [.created(.installed), .failed(.create, .init(.rejection, reason: .destinationExists))])
}

// Failure: the watcher stayed on a templates folder that was moved away, so a replacement
// folder's templates never appeared; activation skips rescans while a watcher exists.
@Test @MainActor func aReplacedTemplatesFolderIsWatchedAgain() async throws {
    let parent = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let root = parent.appendingPathComponent("templates")
    _ = try writeTemplate(named: "before", in: root)
    let store = LocalTemplateStore(templatesURL: root)
    defer { store.stop(); try? SlopPermissions.makeWritable(parent); try? FileManager.default.removeItem(at: parent) }
    await store.refresh()
    #expect(store.templates.map(\.manifest.slug) == ["before"])
    try FileManager.default.moveItem(at: root, to: parent.appendingPathComponent("moved"))
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    // The move is seen: the folder now lists nothing.
    for _ in 0..<100 where !store.templates.isEmpty { try await Task.sleep(for: .milliseconds(20)) }
    #expect(store.templates.isEmpty)
    _ = try writeTemplate(named: "after", in: root)
    for _ in 0..<100 where store.templates.isEmpty {
        await store.refresh(force: false)
        try await Task.sleep(for: .milliseconds(20))
    }
    #expect(store.templates.map(\.manifest.slug) == ["after"])
}

@Test @MainActor func templateFolderChangesRefreshTheExistingStore() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let store = LocalTemplateStore(templatesURL: root)
    defer { store.stop(); try? FileManager.default.removeItem(at: root) }
    await store.refresh()
    #expect(store.templates.isEmpty)
    let package = try writeTemplate(named: "added", in: root)
    await store.refresh()
    #expect(store.templates.map(\.manifest.slug) == ["added"])
    try SlopPermissions.makeWritable(package)
    try FileManager.default.removeItem(at: package)
    await store.refresh()
    #expect(store.templates.isEmpty)
    #expect(store.issues.isEmpty)
}
