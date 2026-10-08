import AppKit
import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopTestSupport
import PDFKit
import Testing

@testable import HitSlopDocument
@testable import HitSlopHost

extension HostTests {
  @Test @MainActor func captureDoesNotTouchTheEditorDuringResize() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let view = controller.session.webView
    _ = try await view.evaluateJavaScript(
      """
      globalThis.editorCaptureCalls = 0;
      __slop.capture.onPrepare(() => { globalThis.editorCaptureCalls++; }); true
      """)
    let capture = Task { try await SlopRenderer.exportPNGData(session: controller.session) }
    controller.window?.setContentSize(NSSize(width: 600, height: 450))
    #expect(NSImage(data: try await capture.value) != nil)
    #expect(try await view.evaluateJavaScript("globalThis.editorCaptureCalls") as? Int == 0)
    #expect(view.frame == controller.window?.contentView?.bounds)
    try await controller.session.close()
    controller.close()
  }

  @Test @MainActor func checklistPDFPreservesSurfaceColorAndSelectableText() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    do {
      #expect(try await command("batch", url: root, setTitle("PDF color")).ok)
      for hex in ["e98996", "80aabb"] {
        #expect(try await setTheme(["surface": "#" + hex], url: root).ok)
        let pdf = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
        #expect(pdf.pageCount == 1)
        #expect(pdf.string?.contains("PDF color") == true)
        let page = try #require(pdf.page(at: 0)?.pageRef)
        let bounds = page.getBoxRect(.mediaBox)
        let context = try #require(
          CGContext(
            data: nil, width: Int(bounds.width), height: Int(bounds.height),
            bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(NSColor.white.cgColor)
        context.fill(bounds)
        context.drawPDFPage(page)
        let rendered = NSBitmapImageRep(cgImage: try #require(context.makeImage()))
        let png = try #require(NSBitmapImageRep(data: try await SlopRenderer.exportPNGData(session: session)))
        #expect(CGFloat(png.pixelsWide) == bounds.width * 2)
        #expect(CGFloat(png.pixelsHigh) == bounds.height * 2)
        // Sample the outer surface, away from text, the paper and its shadow.
        let pdfColor = try #require(rendered.colorAt(x: 4, y: rendered.pixelsHigh / 2)?.usingColorSpace(.sRGB))
        let pngColor = try #require(png.colorAt(x: 8, y: png.pixelsHigh / 2)?.usingColorSpace(.sRGB))
        // Use the working PNG as the color-managed reference, rather than
        // comparing device-RGB captures directly to CSS hex components.
        #expect(min(pdfColor.redComponent, pdfColor.greenComponent, pdfColor.blueComponent) > 0.25)
        #expect(abs(pdfColor.redComponent - pngColor.redComponent) < 0.03)
        #expect(abs(pdfColor.greenComponent - pngColor.greenComponent) < 0.03)
        #expect(abs(pdfColor.blueComponent - pngColor.blueComponent) < 0.03)
        #expect(
          try await session.webView.evaluateJavaScript(
            "!document.documentElement.hasAttribute('data-slop-capture')") as? Bool == true)
      }
      // Capture must restore the editor's decorative background.
      let editorBackground =
        try await session.webView.evaluateJavaScript(
          "getComputedStyle(document.querySelector('.checklist-shell')).backgroundImage") as? String
      #expect(editorBackground?.contains("gradient") == true)
    } catch {
      try await session.close()
      throw error
    }
    try await session.close()
  }

  @Test @MainActor func expiredExportCannotPublishOutput() throws {
    let root = try fixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    let previous = Data("original".utf8)
    try previous.write(to: output)
    #expect(throws: (any Error).self) {
      try SlopRenderer.publishExport(
        Data("late capture".utf8), to: output,
        deadline: NativeCommandDeadline(timeout: .zero))
    }
    #expect(try Data(contentsOf: output) == previous)
  }

  // An export never replaces a file, under any spelling of its path: on a case-insensitive
  // volume another case of the document's name is the document.
  @Test @MainActor func exportNeverReplacesTheDocumentUnderAnotherCase() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let folder = root.deletingLastPathComponent()
    guard
      try folder.resourceValues(forKeys: [.volumeSupportsCaseSensitiveNamesKey]).volumeSupportsCaseSensitiveNames
        == false
    else { return }
    let alias = folder.appendingPathComponent(root.lastPathComponent.lowercased())
    #expect(alias.lastPathComponent != root.lastPathComponent)
    let before = try Data(contentsOf: root)
    #expect(throws: (any Error).self) {
      try SlopRenderer.publishExport(Data("export".utf8), to: alias, deadline: NativeCommandDeadline())
    }
    #expect(try Data(contentsOf: root) == before)
  }

  @Test @MainActor func captureComponentsAreLazyAndUseSavedDocumentWithDefaultView() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let view = session.webView
    let idle =
      "[...document.querySelectorAll('[data-slop-capture-target]')].every(e=>e.hidden && e.childElementCount===0)"
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    let initial = try await savedValue(root)
    let firstIcon = try #require(
      try await SlopRenderer.iconPNGData(session: session))
    let image = try #require(NSBitmapImageRep(data: firstIcon))
    #expect(image.pixelsWide == 512 && image.pixelsHigh == 512)
    #expect(image.hasAlpha)
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    #expect(try await savedValue(root) == initial)
    let rows = try #require(initial?["tasks"] as? [[String: Any]])
    func batch(_ field: String) async throws {
      let ops = rows.map {
        ["type": "set", "path": ["tasks", ["id": $0["$id"]!], field], "value": true] as [String: Any]
      }
      let text = String(decoding: try JSONSerialization.data(withJSONObject: ops), as: UTF8.self)
      #expect(
        try await command(
          "batch", url: root, ["batch": ["intents": try JSONSerialization.jsonObject(with: Data((text).utf8))]]
        ).ok)
    }
    try await batch("done")
    let completedIcon = try #require(
      try await SlopRenderer.iconPNGData(session: session))
    #expect(completedIcon != firstIcon)
    try await batch("archived")
    let filed = try await savedValue(root)
    let filedRows = try #require(filed?["tasks"] as? [[String: Any]])
    #expect(filedRows.count == rows.count)
    #expect(filedRows.allSatisfy { $0["archived"] as? Bool == true })
    let pdf = try await SlopRenderer.exportPDFData(session: session)
    #expect(PDFDocument(data: pdf)?.pageCount ?? 0 > 0)
    #expect(PDFDocument(data: pdf)?.string?.contains("A little breathing room.") == true)
    _ = try await view.evaluateJavaScript(
      "[...document.querySelectorAll('[role=tab]')].find(e => e.textContent.includes('Filed')).click()")
    let filedPDF = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
    #expect(filedPDF.string?.contains("A little breathing room.") == true)
    #expect(filedPDF.string?.contains("Filed tasks") == false)
    #expect(
      try await view.evaluateJavaScript(
        "document.querySelector('[role=tab][aria-selected=true]')?.textContent.includes('Filed')") as? Bool == true)
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    #expect(try await savedValue(root) == filed)
    try await session.close()
    // Background renders read the saved document in place, without owning or writing it.
    let saved = try Data(contentsOf: root)
    #expect(try await SlopRenderer.iconPNGData(url: root) != nil)
    #expect(try Data(contentsOf: root) == saved)
    #expect(try liveDiscovery(path: root.path) == nil)
  }

  // A window writes artwork from its page as it closes, when it changed the document or
  // the file has none; Finder, Quick Look and the catalog then show it as it closed, and
  // Finder's custom icon is a copy of it.
  @Test @MainActor func closingWritesArtworkOfTheChangedDocument() async throws {
    _ = NSApplication.shared
    func closeWindow(_ root: URL, edits ops: String? = nil) async throws {
      // A window captures artwork only once it presented its content.
      let controller = try await SlopDocumentWindowController.open(url: root)
      controller.showWindow(nil)
      await controller.waitForPresentation()
      #expect(controller.isContentReady)
      if let ops {
        #expect(
          try await command(
            "batch", url: root, ["batch": ["intents": try JSONSerialization.jsonObject(with: Data((ops).utf8))]]
          ).ok)
      }
      try await controller.finishClose()
    }
    // A document without artwork gets a preview at its first close.
    let bare = try contractFixture()
    defer { try? FileManager.default.removeItem(at: bare) }
    #expect(SlopArtwork.png(bare, .preview) == nil)
    try await closeWindow(bare)
    #expect(SlopArtwork.png(bare, .preview) != nil)
    // A document with its template's artwork keeps it while unchanged, and gets new artwork,
    // and the matching Finder icon, once a closing window changed it.
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let first = try #require(SlopArtwork.png(root, .preview))
    try await closeWindow(root)
    #expect(SlopArtwork.png(root, .preview) == first, "an unchanged document keeps its artwork")
    #expect(!Fixtures.hasCustomIcon(root))
    try await closeWindow(
      root,
      edits:
        #"[{"type":"insert","path":["tasks"],"value":{"text":"Written as the window closed","done":false,"archived":false}}]"#
    )
    #expect(SlopArtwork.png(root, .preview) != first)
    #expect(Fixtures.hasCustomIcon(root), "Finder shows the new artwork as the file's icon")
  }

  @Test @MainActor func longDocumentPreviewIsCappedWhileExportKeepsFullLength() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let tasks = (0..<300).map { index -> [String: Any] in
      [
        "type": "insert", "path": ["tasks"], "id": String(format: "%026d", index + 1),
        "value": ["text": "Long task \(index)", "done": false, "archived": false],
      ]
    }
    let ops = String(decoding: try JSONSerialization.data(withJSONObject: tasks), as: UTF8.self)
    #expect(
      try await command(
        "batch", url: root, ["batch": ["intents": try JSONSerialization.jsonObject(with: Data((ops).utf8))]]
      ).ok)
    // Full length at 2x exceeds the PNG raster limit; the preview must not.
    let preview = try #require(NSBitmapImageRep(data: try await SlopRenderer.previewPNGData(url: root)))
    #expect(preview.pixelsWide == 960 && preview.pixelsHigh == 960 * 3)  // 480pt wide at 2x, capped at 3:1
    let pdf = try #require(
      PDFDocument(
        data: try await SlopRenderer.withRenderSession(url: root) {
          try await SlopRenderer.exportPDFData(session: $0)
        }))
    let page = try #require(pdf.page(at: 0))
    #expect(page.bounds(for: .mediaBox).height > 5000)
  }

  @Test @MainActor func captureFailureLeavesEditorAndSavedArtworkAlone() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      let artwork = stage.appendingPathComponent("artwork", isDirectory: true)
      try FileManager.default.createDirectory(at: artwork, withIntermediateDirectories: true)
      try Fixtures.addArtwork(stage, name: "preview", bytes: Fixtures.png())
      let app = stage.appendingPathComponent("assets/ui.js")
      let script = try String(contentsOf: app, encoding: .utf8).replacingOccurrences(
        of: "const doc = ctx.document;",
        with:
          "const doc = ctx.document; ctx.capture.onPrepare(() => { if (ctx.capture.isRenderer()) throw new Error('capture test failure'); });"
      )
      try Data(script.utf8).write(to: app)
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let oldPreview = try #require(await session.artwork(.preview))
    await #expect(throws: (any Error).self) {
      try await SlopRenderer.exportPNGData(session: session)
    }
    let artwork = await SlopRenderer.artwork(session: session, telemetry: SlopTelemetry { _ in })
    #expect(artwork.preview == nil && artwork.icon == nil)
    #expect(session.capturing == false)
    #expect(
      try await session.webView.evaluateJavaScript(
        "!document.documentElement.hasAttribute('data-slop-capture')") as? Bool == true)
    try await session.close(artwork: artwork)
    #expect(SlopArtwork.png(root, .preview) == oldPreview)
  }

  @Test @MainActor func savedCaptureSourceSurvivesEditorChangesAndClose() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    // Attached before the draft replaces the title: the capture source still holds the
    // blob, though nothing references it once the draft is saved.
    let blob = try Fixtures.png()
    let id = try await attach(blob, at: ["title"], url: root)
    _ = try await session.webView.evaluateJavaScript(
      """
      const input = document.querySelector('#draft'); input.value = 'Snapshot draft';
      input.dispatchEvent(new Event('input', {bubbles:true})); true
      """)
    var sourceURL: URL?
    try await session.withCaptureSnapshot { source in
      sourceURL = source
      #expect(!session.capturing)
      #expect(try await command("batch", url: root, setTitle("Later edit")).ok)
      try await session.close()
      let read = try await command("attachments.read", url: source, ["attachmentID": id])
      #expect((read.state as? [String: Any])?["bytes"] as? String == blob.base64EncodedString())
      let pdf = try #require(
        PDFDocument(
          data: try await SlopRenderer.withRenderSession(url: source) {
            try await SlopRenderer.exportPDFData(session: $0)
          }))
      #expect(pdf.string?.contains("Snapshot draft") == true)
      #expect(pdf.string?.contains("Later edit") == false)
      #expect(try await SlopRenderer.iconPNGData(url: source) == nil)
    }
    #expect(!FileManager.default.fileExists(atPath: try #require(sourceURL).path))
    #expect(try await savedValue(root)?["title"] as? String == "Later edit")
  }

  @Test @MainActor func failedSaveDoesNotAcquireCaptureSourceOrReleaseOwnership() async throws {
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    let hold = try Fixtures.DatabaseHold(root)
    _ = try await session.owner.apply(
      batch: Fixtures.json([
        "intents": [
          [
            "type": "set", "path": ["title"], "value": "Unsaved edit",
          ]
        ]
      ]))
    var rendered = false
    await #expect(throws: (any Error).self) {
      try await session.withCaptureSnapshot { _ in rendered = true }
    }
    #expect(!rendered && !session.capturing)
    #expect(Fixtures.isLocked(root))
    hold.release()
    try await session.close()
  }

  @Test @MainActor func missingExportUsesFreshReadOnlyApp() async throws {
    let root = try contractFixture { stage in
      let script = """
        export default { mount(ctx, target) {
          const root = document.createElement('main'); root.dataset.hitslopRoot = '';
          root.textContent = 'Fallback: ' + ctx.document.current.title;
          target.append(root); return { unmount() { root.remove(); } };
        } };
        """
      try Data(script.utf8).write(to: stage.appendingPathComponent("assets/ui.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    #expect(try await command("batch", url: root, setTitle("Saved fallback")).ok)
    let pdf = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
    #expect(pdf.string?.contains("Fallback: Saved fallback") == true)
    try await session.close()
  }

}

extension HostTests {
  @Test @MainActor func telemetryCountsCompletedExportsAndReportsFailuresWithoutDocumentValues() async throws {
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    var events: [SlopTelemetryEvent] = []
    let controller = try await SlopDocumentWindowController.open(
      url: root,
      telemetry: SlopTelemetry {
        if case .breadcrumb = $0 { return }
        events.append($0)
      })
    try await controller.session.waitUntilReady()
    try await controller.exportDocument(format: .pdf, to: nil)
    #expect(events.isEmpty)
    try await controller.exportDocument(format: .pdf, to: output)
    #expect(PDFDocument(data: try Data(contentsOf: output)) != nil)
    #expect(events == [.exported(.pdf)])
    await #expect(throws: (any Error).self) {
      // The destination is the document itself: refused before anything is written.
      try await controller.exportDocument(format: .png, to: root)
    }
    #expect(events == [.exported(.pdf), .failed(.export, .init(.rejection, reason: .operationRejected, format: .png))])
    // A CLI export of the open document is the window's export.
    let liveExport = try #require(controller.session.onExport)
    await #expect(throws: (any Error).self) {
      try await liveExport(.png, root, .init())
    }
    #expect(events.count == 3)
    #expect(events.last == .failed(.export, .init(.rejection, reason: .operationRejected, format: .png)))
    try await controller.session.close()
  }
}
