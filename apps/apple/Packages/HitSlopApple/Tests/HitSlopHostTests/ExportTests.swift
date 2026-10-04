import AppKit
import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import PDFKit
import Testing
import HitSlopTestSupport

@testable import HitSlopHost
@testable import HitSlopDocument

extension HostTests {
  @Test @MainActor func captureRestoresCurrentWindowSizeAfterConcurrentResize() async throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    try await controller.session.waitUntilReady()
    let view = controller.session.webView
    _ = try await view.evaluateJavaScript("""
      globalThis.stopResizeProbe = __slop.capture.onPrepare(() => new Promise(resolve => {
        globalThis.releaseResizeProbe = resolve;
      })); true
      """)
    let capture = Task { try await SlopRenderer.exportPNGData(session: controller.session) }
    var prepared = false
    for _ in 0..<100 {
      prepared = try await view.evaluateJavaScript("typeof globalThis.releaseResizeProbe === 'function'") as? Bool == true
      if prepared { break }
      try await Task.sleep(for: .milliseconds(10))
    }
    #expect(prepared)
    controller.window?.setContentSize(NSSize(width: 600, height: 450))
    _ = try await view.evaluateJavaScript("globalThis.releaseResizeProbe?.(); globalThis.stopResizeProbe(); true")
    _ = try await capture.value
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
        #expect(try await command("theme.set", url: root, ["values": ["surface": "#" + hex]]).ok)
        let pdf = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
        #expect(pdf.pageCount == 1)
        #expect(pdf.string?.contains("PDF color") == true)
        let page = try #require(pdf.page(at: 0)?.pageRef)
        let bounds = page.getBoxRect(.mediaBox)
        let context = try #require(CGContext(data: nil, width: Int(bounds.width), height: Int(bounds.height),
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
        #expect(try await session.webView.evaluateJavaScript(
          "!document.documentElement.hasAttribute('data-slop-capture')") as? Bool == true)
      }
      // Capture must restore the editor's decorative background.
      let editorBackground = try await session.webView.evaluateJavaScript(
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
        Data("late capture".utf8), to: output, source: root,
        deadline: NativeCommandDeadline(timeout: .zero))
    }
    #expect(try Data(contentsOf: output) == previous)
  }

  @Test @MainActor func captureComponentsAreLazyAndUseCurrentDocumentAndSelectedView() async throws {
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
      let ops = rows.map { ["type": "set", "path": ["tasks", ["id": $0["$id"]!], field], "value": true] as [String: Any] }
      let text = String(decoding: try JSONSerialization.data(withJSONObject: ops), as: UTF8.self)
      #expect(try await command("batch", url: root, ["ops": text]).ok)
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
    _ = try await view.evaluateJavaScript("[...document.querySelectorAll('[role=tab]')].find(e => e.textContent.includes('Filed')).click()")
    let filedPDF = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
    #expect(filedPDF.string?.contains("Filed tasks") == true)
    for row in rows {
      #expect(filedPDF.string?.contains(row["text"] as? String ?? "") == true)
    }
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
  // the file has none; Finder, Quick Look and the catalog then show it as it closed.
  @Test @MainActor func closingWritesArtworkOfTheChangedDocument() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    func closeWindow(editing title: String?) async throws {
      let controller = try await SlopDocumentWindowController.open(url: root)
      try await controller.session.waitUntilReady()
      if let title {
        let op = [["type": "insert", "path": ["tasks"], "value": ["text": title, "done": false, "archived": false]]]
        let text = String(decoding: try JSONSerialization.data(withJSONObject: op), as: UTF8.self)
        #expect(try await command("batch", url: root, ["ops": text]).ok)
      }
      try await controller.finishClose()
    }
    try await closeWindow(editing: nil)
    let first = try #require(SlopArtwork.png(root, .preview), "a file without a preview gets one")
    #expect(SlopArtwork.png(root, .icon) != nil)
    try await closeWindow(editing: nil)
    #expect(SlopArtwork.png(root, .preview) == first, "an unchanged document keeps its artwork")
    try await closeWindow(editing: "Written as the window closed")
    #expect(SlopArtwork.png(root, .preview) != first)
  }

  @Test @MainActor func longDocumentPreviewIsCappedWhileExportKeepsFullLength() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let tasks = (0..<300).map { index -> [String: Any] in
      ["type": "insert", "path": ["tasks"], "id": String(format: "%026d", index + 1), "value": ["text": "Long task \(index)", "done": false, "archived": false]]
    }
    let ops = String(decoding: try JSONSerialization.data(withJSONObject: tasks), as: UTF8.self)
    #expect(try await command("batch", url: root, ["ops": ops]).ok)
    // Full length at 2x exceeds the PNG raster limit; the preview must not.
    let preview = try #require(NSBitmapImageRep(data: try await SlopRenderer.previewPNGData(url: root)))
    #expect(preview.pixelsWide == 960 && preview.pixelsHigh == 960 * 3)  // 480pt wide at 2x, capped at 3:1
    let pdf = try #require(PDFDocument(data: try await SlopRenderer.withRenderSession(url: root) {
      try await SlopRenderer.exportPDFData(session: $0)
    }))
    let page = try #require(pdf.page(at: 0))
    #expect(page.bounds(for: .mediaBox).height > 5000)
  }

  @Test @MainActor func captureFailureRestoresEditorAndMissingIconIsOptional() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    #expect(try await SlopRenderer.iconPNGData(session: session) == nil)
    _ = try await session.webView.callAsyncJavaScript(
      "globalThis.stopFailure=globalThis.__slop.capture.onPrepare(()=>{throw new Error('capture test failure')});return true",
      arguments: [:], in: nil, contentWorld: .page)
    await #expect(throws: (any Error).self) {
      try await SlopRenderer.exportPNGData(session: session)
    }
    #expect(session.capturing == false)
    #expect(
      try await session.webView.evaluateJavaScript(
        "!document.documentElement.hasAttribute('data-slop-capture')") as? Bool == true)
    _ = try await session.webView.evaluateJavaScript("globalThis.stopFailure()")
    #expect(NSImage(data: try await SlopRenderer.exportPNGData(session: session)) != nil)
    try await session.close()
  }

  // A queued capture takes over without clearing `capturing`, so no page or socket
  // request runs between two captures.
  @Test @MainActor func queuedCapturesKeepTheSessionCapturingAcrossTheHandOff() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    // The watcher outranks the captures, so it runs before a resumed capture would.
    var finished = 0, gaps = 0
    let captures = (0..<2).map { _ in
      Task(priority: .background) { @MainActor in
        defer { finished += 1 }
        return try await SlopRenderer.previewPNGData(session: session)
      }
    }
    var started = false
    while finished < 2 {
      if session.capturing { started = true } else if started { gaps += 1 }
      await Task.yield()
    }
    for capture in captures { _ = try await capture.value }
    #expect(gaps == 0)
    #expect(session.capturing == false)
    try await session.close()
  }
}

extension HostTests {
  @Test @MainActor func telemetryCountsCompletedExportsAndReportsFailuresWithoutDocumentValues() async throws {
    let root = try contractFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: output) }
    var events: [SlopTelemetryEvent] = []
    let controller = try await SlopDocumentWindowController.open(
      url: root, telemetry: SlopTelemetry { if case .breadcrumb = $0 { return }; events.append($0) })
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
