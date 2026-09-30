import AppKit
import Foundation
import HitSlopCore
import HitSlopRuntime
import PDFKit
import Testing

@testable import HitSlopHost
@testable import HitSlopDocument

extension LoroClientTests {
  @Test @MainActor func checklistPDFPreservesSurfaceColorAndSelectableText() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await SlopRuntimeSession.open(packageURL: root)
    session.load()
    try await session.waitUntilReady()
    do {
      _ = try await DocumentCommand.run(method: "apply", url: root, operation: setTitle("PDF color"))
      for hex in ["e98996", "80aabb"] {
        _ = try await DocumentCommand.run(method: "theme.set", url: root,
          themeValues: JSONSerialization.data(withJSONObject: ["surface": "#" + hex]))
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
      try await session.finish()
      throw error
    }
    try await session.finish()
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

  @Test @MainActor func inlineCaptureTargetsAreLazyAndUseCurrentDocument() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await SlopRuntimeSession.open(packageURL: root)
    session.load()
    try await session.waitUntilReady()
    let view = session.webView
    let idle =
      "[...document.querySelectorAll('[data-slop-capture-target]')].every(e=>e.hidden && e.childElementCount===0)"
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    let initial = try await DocumentCommand.run(method: "get", url: root)
    let firstIcon = try #require(
      try await SlopRenderer.targetPNGData(session: session, target: .icon))
    let image = try #require(NSBitmapImageRep(data: firstIcon))
    #expect(image.pixelsWide == 512 && image.pixelsHigh == 512)
    #expect(image.hasAlpha)
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    #expect(try await DocumentCommand.run(method: "get", url: root) == initial)
    let state = try #require(try JSONSerialization.jsonObject(with: initial) as? [String: Any])
    let rows = try #require(state["tasks"] as? [[String: Any]])
    let operations = rows.map {
      ["type": "set", "path": ["tasks", ["id": $0["$id"]!], "done"], "value": true] as [String: Any]
    }
    _ = try await DocumentCommand.run(
      method: "batch", url: root, operations: JSONSerialization.data(withJSONObject: operations))
    let completedIcon = try #require(
      try await SlopRenderer.targetPNGData(session: session, target: .icon))
    #expect(completedIcon != firstIcon)
    let archiveOperations = rows.map {
      ["type": "set", "path": ["tasks", ["id": $0["$id"]!], "archived"], "value": true] as [String: Any]
    }
    _ = try await DocumentCommand.run(method: "batch", url: root, operations: JSONSerialization.data(withJSONObject: archiveOperations))
    let filed = try await DocumentCommand.run(method: "get", url: root)
    let filedState = try #require(try JSONSerialization.jsonObject(with: filed) as? [String: Any])
    let filedRows = try #require(filedState["tasks"] as? [[String: Any]])
    #expect(filedRows.count == rows.count)
    #expect(filedRows.allSatisfy { $0["archived"] as? Bool == true })
    let pdf = try await SlopRenderer.exportPDFData(session: session)
    #expect(PDFDocument(data: pdf)?.pageCount ?? 0 > 0)
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    #expect(try await DocumentCommand.run(method: "get", url: root) == filed)
    try await session.finish()
    let stateDirectory = root.appendingPathComponent("state")
    func stateFiles() throws -> [String: Data] {
      try Dictionary(uniqueKeysWithValues: FileManager.default.contentsOfDirectory(atPath: stateDirectory.path)
        .map { ($0, try Data(contentsOf: stateDirectory.appendingPathComponent($0))) })
    }
    let saved = try stateFiles()
    let assets = try await SlopRenderer.documentAssetsPNGData(packageURL: root)
    // Background renders read the saved document in place without copying or writing it.
    #expect(try stateFiles() == saved)
    #expect(!FileManager.default.fileExists(atPath: stateDirectory.appendingPathComponent("host.lock").path))
    let preview = try #require(assets.previewPNG)
    let icon = try #require(assets.finderIconPNG)
    try SlopPreviewWriter.write(preview, to: root)
    SlopPreviewWriter.installFinderIcon(icon, for: root)
    #expect(
      FileManager.default.fileExists(
        atPath: root.appendingPathComponent("QuickLook/Preview.png").path))
    #expect(FileManager.default.fileExists(atPath: root.appendingPathComponent("Icon\r").path))
    #expect(try Data(contentsOf: root.appendingPathComponent("QuickLook/Icon.png")) != icon)
  }

  @Test @MainActor func longDocumentPreviewIsCappedWhileExportKeepsFullLength() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let tasks = (0..<300).map { index -> [String: Any] in
      ["type": "insert", "path": ["tasks"], "id": String(format: "%026d", index + 1), "value": ["text": "Long task \(index)", "done": false, "archived": false]]
    }
    _ = try await DocumentCommand.run(
      method: "batch", url: root, operations: JSONSerialization.data(withJSONObject: tasks))
    // Full length at 2x exceeds the PNG raster limit; the preview must not.
    let preview = try #require(NSBitmapImageRep(data: try await SlopRenderer.previewPNGData(packageURL: root)))
    #expect(preview.pixelsWide == 960 && preview.pixelsHigh == 960 * 3)  // 480pt wide at 2x, capped at 3:1
    let pdf = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(packageURL: root)))
    let page = try #require(pdf.page(at: 0))
    #expect(page.bounds(for: .mediaBox).height > 5000)
  }

  @Test @MainActor func renderSnapshotIsTakenUnderOwnershipAndRendersWithoutIt() async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    // As in closed-document export: ownership is held until the snapshot exists.
    var ownership: DocumentWriterLock? = try DocumentWriterLock(root: root)
    let pdf = try await SlopRenderer.withRenderSession(
      packageURL: root,
      inputReady: {
        #expect(throws: (any Error).self) { try DocumentWriterLock(root: root) }
        ownership?.close()
        ownership = nil
      }
    ) { session in
      // Another writer may open the document while it renders from memory.
      try DocumentWriterLock(root: root).close()
      return try await SlopRenderer.exportPDFData(session: session)
    }
    #expect(ownership == nil)
    #expect(PDFDocument(data: pdf)?.pageCount ?? 0 > 0)
  }

  @Test @MainActor func captureFailureRestoresEditorAndMissingIconIsOptional() async throws {
    _ = NSApplication.shared
    let root = try captureFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await SlopRuntimeSession.open(packageURL: root)
    session.load()
    try await session.waitUntilReady()
    #expect(try await SlopRenderer.targetPNGData(session: session, target: .icon) == nil)
    _ = try await session.webView.callAsyncJavaScript(
      "globalThis.stopFailure=globalThis.__hitslopCapture.onPrepare(()=>{throw new Error('capture test failure')});return true",
      arguments: [:], in: nil, contentWorld: .page)
    await #expect(throws: (any Error).self) {
      try await SlopRenderer.exportPNGData(session: session)
    }
    #expect(session.engine.capturing == false)
    #expect(
      try await session.webView.evaluateJavaScript(
        "!document.documentElement.hasAttribute('data-slop-capture')") as? Bool == true)
    _ = try await session.webView.evaluateJavaScript("globalThis.stopFailure()")
    #expect(NSImage(data: try await SlopRenderer.exportPNGData(session: session)) != nil)
    try await session.finish()
  }
}

extension LoroClientTests {
  @Test @MainActor func telemetryCountsCompletedExportsAndReportsFailuresWithoutDocumentValues() async throws {
    let root = try captureFixture()
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: output) }
    let controller = try await SlopDocumentWindowController.open(packageURL: root)
    try await controller.session.waitUntilReady()
    var events: [SlopTelemetryEvent] = []
    controller.telemetry = SlopTelemetry { if case .breadcrumb = $0 { return }; events.append($0) }
    try await controller.exportDocument(format: .pdf, to: nil)
    #expect(events.isEmpty)
    try await controller.exportDocument(format: .pdf, to: output)
    #expect(PDFDocument(data: try Data(contentsOf: output)) != nil)
    #expect(events == [.exported(.pdf)])
    await #expect(throws: (any Error).self) {
      try await controller.exportDocument(format: .png, to: root.appendingPathComponent("private-name.png"))
    }
    #expect(events == [.exported(.pdf), .failed(.export, .init(.rejection, reason: .operationRejected, format: .png))])
    // The app-hosted CLI export callback must use the same reporting boundary.
    let liveExport = try #require(controller.session.engine.onExport)
    await #expect(throws: (any Error).self) {
      try await liveExport("png", root.appendingPathComponent("private-cli-export.png"), .init())
    }
    #expect(events.count == 3)
    #expect(events.last == .failed(.export, .init(.rejection, reason: .operationRejected,
      format: .png)))
    try await controller.session.finish()
  }
}
