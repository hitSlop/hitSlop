import AppKit
import ComposableArchitecture
import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopHost
import HitSlopDocument
import Testing
import HitSlopTestSupport
import HitSlopCoreBinding
import SQLite3
import SwiftUI
@testable import HitSlopCatalog

// Same-path artwork replacement must update an already mounted catalog, without reselection.
@Test(arguments: [false, true]) @MainActor func displayedCatalogArtworkRefreshesAfterSamePathReplacement(icon: Bool) async throws {
    _ = NSApplication.shared
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    let red = try coloredArtwork(.red)
    let template = try writeTemplate(named: "preview-refresh", in: root, preview: red, icon: red)
    let file = root.appendingPathComponent("document.slop")
    _ = try SlopFile.create(from: template, to: file)
    try FileManager.default.setAttributes([.modificationDate: Date(timeIntervalSince1970: 100)], ofItemAtPath: file.path)
    let scanner = CatalogScanner()
    var initial = CatalogFeature.State()
    initial.isStarted = true
    initial.filter = .recents
    initial.recents = try await scanner.recents([file])
    initial.selectedID = try #require(initial.recents.first).id
    let store = Store(initialState: initial) { CatalogFeature() } withDependencies: {
        $0.catalogClient.recents = { (try? await scanner.recents([file])) ?? [] }
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
    let blue = try coloredArtwork(.blue)
    try await writeArtwork(file, preview: icon ? nil : blue, icon: icon ? blue : nil)
    try await expectPreview(in: host, blue: true, minimumPixels: icon ? 30 : 100)
    #expect(store.selectedID == initial.selectedID)
}

private func coloredArtwork(_ color: NSColor) throws -> Data {
    try Fixtures.png(width: 512, height: 512) { color.setFill(); $0.fill() }
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

@Test @MainActor func discoversAndCreatesFromInstalledTemplate() async throws {
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    let file = try writeTemplate(named: "tiny-counter", in: root, categories: ["utilities", "other"])

    let store = LocalTemplateStore(templatesURL: root)
    await store.refresh()
    #expect(store.templates.count == 1)
    #expect(store.templates.first?.manifest.title == "Tiny Counter")
    #expect(store.templates.first?.icon.name == .icon)
    #expect(store.issues.isEmpty)

    let destination = root.appendingPathComponent("created.slop")
    _ = try SlopFile.create(from: #require(store.templates.first).url, to: destination)
    #expect(try SlopFile(url: destination).manifest.categories == [.utilities, .other])
    #expect(SlopArtwork.png(destination, .preview) == png)
    #expect(SlopArtwork.png(destination, .icon) == iconPNG)
    let updatedPreview = try coloredArtwork(.red)
    try await writeArtwork(destination, preview: updatedPreview)
    #expect(SlopArtwork.png(destination, .preview) == updatedPreview)
    #expect(SlopArtwork.png(destination, .icon) == iconPNG)
    let updatedIcon = try coloredArtwork(.blue)
    try await writeArtwork(destination, icon: updatedIcon)
    #expect(SlopArtwork.png(destination, .icon) == updatedIcon)
    // The template keeps its own artwork.
    #expect(SlopArtwork.png(file, .icon) == iconPNG)
}

@Test @MainActor func discoversOnlyTopLevelPackages() async throws {
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    _ = try writeTemplate(named: "tiny-counter", in: root)
    _ = try writeTemplate(named: "cached-slop", in: root.appendingPathComponent("nested/archived/cached-slop", isDirectory: true), fileName: "1.slop")

    let store = LocalTemplateStore(templatesURL: root)
    await store.refresh()
    #expect(store.templates.map(\.manifest.slug) == ["tiny-counter"])
    #expect(store.issues.isEmpty)
}

private let png = try! Fixtures.png(width: 2, height: 2)
private let iconPNG = try! Fixtures.png(width: 512, height: 512)

/// An installed template built the way `slop build` builds one, with preview and icon artwork.
private func writeTemplate(
    named slug: String, in directory: URL, fileName: String? = nil, categories: [String] = ["utilities", "personal"],
    preview: Data = png, icon: Data = iconPNG
) throws -> URL {
    let stage = try Fixtures.minimalStage(slug: slug, manifest: ["title": "Tiny Counter", "categories": categories])
    defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
    try FileManager.default.createDirectory(at: stage.appendingPathComponent("artwork"), withIntermediateDirectories: true)
    try preview.write(to: stage.appendingPathComponent("artwork/preview.png"))
    try icon.write(to: stage.appendingPathComponent("artwork/icon.png"))
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let template = directory.appendingPathComponent(fileName ?? "\(slug).slop")
    try FileManager.default.moveItem(at: Fixtures.template(stage: stage), to: template)
    return template
}

/// Writes artwork into a document as a closing window does, and announces it.
@MainActor private func writeArtwork(_ document: URL, preview: Data? = nil, icon: Data? = nil) async throws {
    try await DocumentOwner(url: document).close(artwork: SlopRenderedArtwork(preview: preview, icon: icon))
    SlopPreviewWriter.announce(document)
}

// An installed template is validated again once its file changes.
@Test func aChangedTemplateIsValidatedAgain() async throws {
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    let file = try writeTemplate(named: "cached", in: root)
    let scanner = CatalogScanner()
    #expect(try await scanner.local(at: root).templates.count == 1)
    var db: OpaquePointer?
    #expect(sqlite3_open(file.path, &db) == SQLITE_OK)
    #expect(sqlite3_exec(db, "UPDATE app SET manifest = substr(manifest, 2)", nil, nil, nil) == SQLITE_OK)
    sqlite3_close(db)
    try FileManager.default.setAttributes([.modificationDate: Date().addingTimeInterval(60)], ofItemAtPath: file.path)
    let rescanned = try await scanner.local(at: root)
    #expect(rescanned.templates.isEmpty)
    #expect(rescanned.issues.count == 1)
}

@Test @MainActor func templateIdentityIncludesItsSource() async throws {
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    for source in ["built-in", "registered"] { _ = try writeTemplate(named: "same-slug", in: root.appendingPathComponent(source)) }
    let scanner = CatalogScanner()
    let builtIn = try await scanner.local(at: root.appendingPathComponent("built-in"))
    let local = try await scanner.local(at: root.appendingPathComponent("registered"))
    let first = CatalogServices.localEntry(try #require(builtIn.templates.first))
    let second = CatalogServices.localEntry(try #require(local.templates.first))
    #expect(first.id != second.id)
}

@Test @MainActor func localCatalogCombinesSourcesAndReportsInvalidPackages() async throws {
    let root = try Fixtures.folder()
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
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    let templates = root.appendingPathComponent("templates")
    let source = try writeTemplate(named: "tiny-counter", in: templates)
    let before = try Data(contentsOf: source)
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
    #expect(try Data(contentsOf: source) == before)
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
    defer { store.stop(); try? FileManager.default.removeItem(at: parent) }
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
    let file = try writeTemplate(named: "added", in: root)
    await store.refresh()
    #expect(store.templates.map(\.manifest.slug) == ["added"])
    try FileManager.default.removeItem(at: file)
    await store.refresh()
    #expect(store.templates.isEmpty)
    #expect(store.issues.isEmpty)
}

/// A cold catalog scan of 20 built Quick Checklist templates: every file checked, as at
/// launch. Median of 10 scans, each by a new scanner (no version cache).
@Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_BENCH_OPEN"] == "1"))
func catalogScanCost() async throws {
  let built = Fixtures.repository.appendingPathComponent("generated/templates/quick-checklist.slop")
  let root = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: root) }
  for index in 0..<20 {
    try FileManager.default.copyItem(at: built, to: root.appendingPathComponent("checklist-\(index).slop"))
  }
  var times: [Double] = []
  for _ in 0..<10 {
    let start = DispatchTime.now().uptimeNanoseconds
    _ = try await CatalogScanner().local(at: root)
    times.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
  }
  print("SCAN-BENCH {\"templates\":20,\"median-ms\":\(times.sorted()[times.count / 2])}")
}
