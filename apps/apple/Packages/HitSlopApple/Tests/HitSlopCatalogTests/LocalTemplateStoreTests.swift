import AppKit
import ComposableArchitecture
import Foundation
import HitSlopCore
import HitSlopHost
import HitSlopRuntime
import Testing
import SwiftUI
@testable import HitSlopCatalog

// Same-path artwork replacement must update an already mounted catalog, without reselection.
@Test @MainActor func displayedCatalogPreviewRefreshesAfterSamePathReplacement() async throws {
    _ = NSApplication.shared
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try writeTemplate(named: "preview-refresh", in: root)
    let preview = package.appendingPathComponent("QuickLook/Preview.png")
    try coloredPreview(.red).write(to: preview)
    try FileManager.default.setAttributes([.modificationDate: Date(timeIntervalSince1970: 100)], ofItemAtPath: preview.path)
    let scanner = CatalogScanner()
    let templates = root.appendingPathComponent("templates")
    var initial = CatalogFeature.State()
    initial.isStarted = true
    initial.filter = .recents
    initial.recents = try await scanner.recents([package], templatesRoot: templates)
    initial.selectedID = try #require(initial.recents.first).id
    let store = Store(initialState: initial) { CatalogFeature() } withDependencies: {
        $0.catalogClient.recents = { (try? await scanner.recents([package], templatesRoot: templates)) ?? [] }
        $0.catalogClient.refreshLocal = {}
    }
    let host = NSHostingView(rootView: CatalogView(store: store))
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720),
        styleMask: [.titled], backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    window.contentView = host
    window.orderFront(nil)
    defer { window.close() }
    try await expectPreview(in: host, blue: false)
    try SlopPreviewWriter.write(coloredPreview(.blue), to: package)
    try await expectPreview(in: host, blue: true)
    #expect(store.selectedID == initial.selectedID)
}

@MainActor private func coloredPreview(_ color: NSColor) throws -> Data {
    let bitmap = try #require(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 320, pixelsHigh: 200,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    color.setFill()
    NSRect(x: 0, y: 0, width: 320, height: 200).fill()
    NSGraphicsContext.restoreGraphicsState()
    return try #require(bitmap.representation(using: .png, properties: [:]))
}

@MainActor private func expectPreview(in view: NSView, blue: Bool) async throws {
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
            if pixels > 100 { return }
        }
        // SwiftUI exposes no completion callback for display; wait for the rendered fixture color.
        try await Task.sleep(for: .milliseconds(20))
    } while ContinuousClock.now < deadline
    Issue.record("Catalog did not display the \(blue ? "replacement blue" : "initial red") preview")
}

@Test @MainActor func discoversAndDuplicatesInstalledTemplate() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let package = try writeTemplate(named: "tiny-counter", in: root)

    let store = LocalTemplateStore(templatesURL: root)
    await store.refresh()
    #expect(store.templates.count == 1)
    #expect(store.templates.first?.manifest.title == "Tiny Counter")
    #expect(store.templates.first?.icon.url.lastPathComponent == "Icon.png")
    #expect(store.issues.isEmpty)

    let destination = root.appendingPathComponent("created.slop", isDirectory: true)
    try DocumentFactory().create(fromLocalPackage: #require(store.templates.first).packageURL, at: destination)
    SlopPreviewWriter.installExistingPreview(for: destination)
    #expect(try Data(contentsOf: package.appendingPathComponent("manifest.json")) == Data(contentsOf: destination.appendingPathComponent("manifest.json")))
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("assets/app.js").path))
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("QuickLook/Preview.png").path))
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("QuickLook/Icon.png").path))
    let updatedPreview = Data("updated preview".utf8)
    try SlopPreviewWriter.write(updatedPreview, to: destination)
    #expect(try Data(contentsOf: destination.appendingPathComponent("QuickLook/Preview.png")) == updatedPreview)
    #expect(try Data(contentsOf: destination.appendingPathComponent("QuickLook/Icon.png")) == iconPNG)
    #expect(FileManager.default.fileExists(atPath: destination.appendingPathComponent("Icon\r").path))
    SlopPreviewWriter.installFinderIcon(iconPNG, for: destination)
    #expect(try Data(contentsOf: destination.appendingPathComponent("QuickLook/Icon.png")) == iconPNG)
    #expect(!FileManager.default.fileExists(atPath: package.appendingPathComponent("Icon\r").path))
    #expect(!FileManager.default.fileExists(atPath: destination.appendingPathComponent("stores").path))
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
    try Data(#"{"format":1,"root":{"kind":"object","properties":{}}}"#.utf8).write(to: package.appendingPathComponent("state.schema.json"))
    try Data("{}".utf8).write(to: package.appendingPathComponent("initial.json"))
    let manifest = #"{"runtime":"hitslop-v1","$schema":"https://api.hitslop.com/schemas/v1/manifest.schema.json","author":{"name":"Fixture Author","url":"https://example.com"},"slug":"\#(slug)","title":"Tiny Counter","description":"Counts a very small thing.","categories":["utilities","personal"],"presentation":{"width":320,"height":240}}"#
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
    #expect(entries.allSatisfy { $0.categories == ["utilities", "personal"] })
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

@Test @MainActor func templateFolderChangesRefreshTheExistingStore() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let store = LocalTemplateStore(templatesURL: root)
    defer { store.stop(); try? FileManager.default.removeItem(at: root) }
    await store.refresh()
    #expect(store.templates.isEmpty)
    let package = try writeTemplate(named: "added", in: root)
    await store.refresh()
    #expect(store.templates.map(\.manifest.slug) == ["added"])
    try SlopDuplicator.makeWritable(package)
    try FileManager.default.removeItem(at: package)
    await store.refresh()
    #expect(store.templates.isEmpty)
    #expect(store.issues.isEmpty)
}
