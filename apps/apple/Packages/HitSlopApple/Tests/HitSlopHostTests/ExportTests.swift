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
  @Test @MainActor func screenshotPublicationProtectsDestinationsAndReplacesPNGs() throws {
    let folder = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: folder) }
    let source = folder.appendingPathComponent("source.slop")
    let output = folder.appendingPathComponent("screenshot.png")
    let red = try Fixtures.png(width: 8, height: 8) {
      NSColor.red.setFill()
      $0.fill()
    }
    let blue = try Fixtures.png(width: 8, height: 8) {
      NSColor.blue.setFill()
      $0.fill()
    }
    try SlopScreenshotDestination(output: output, source: source).publish(red)
    try SlopScreenshotDestination(output: output, source: source).publish(blue)
    #expect(try Data(contentsOf: output) == blue)
    let pending = try SlopScreenshotDestination(output: output, source: source)
    try red.write(to: output, options: .atomic)
    #expect(throws: (any Error).self) { try pending.publish(blue) }
    #expect(try Data(contentsOf: output) == red)
    let new = folder.appendingPathComponent("new.png")
    let absent = try SlopScreenshotDestination(output: new, source: source)
    try blue.write(to: new)
    #expect(throws: (any Error).self) { try absent.publish(red) }
    #expect(try Data(contentsOf: new) == blue)
    let link = folder.appendingPathComponent("link.png")
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: output)
    #expect(throws: (any Error).self) { _ = try SlopScreenshotDestination(output: link, source: source) }
    let text = folder.appendingPathComponent("text.png")
    try Data("Keep this file".utf8).write(to: text)
    #expect(throws: (any Error).self) { _ = try SlopScreenshotDestination(output: text, source: source) }
  }

  @Test(.nightly) @MainActor func largestDefaultPreviewStaysWithinRasterBudget() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      try Fixtures.updateApp(stage) {
        $0["window"] = ["kind": "standard", "width": 4096, "height": 4096]
      }
      try Fixtures.writeApp(
        "export default { mount(ctx, target) { target.textContent = 'Large preview'; } };", to: stage)
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let preview = try #require(
      NSBitmapImageRep(data: try await SlopRenderer.capture(url: root, output: .previewPNG, provider: .webKit)))
    #expect(preview.pixelsWide == 4096 && preview.pixelsHigh == 4096)
    #expect(preview.pixelsWide * preview.pixelsHigh <= Limits.imagePixels)
  }

  @Test(.nightly) @MainActor func dedicatedExportCanBeWiderThanItsWindow() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      try Fixtures.updateApp(stage) {
        $0["window"] = ["kind": "standard", "width": 280, "height": 300]
      }
      try Fixtures.writeApp(
        """
        export default { mount(ctx, target) {
          const surface = document.createElement('section');
          surface.style.cssText = 'width:520px;height:160px;background:white;position:relative';
          surface.innerHTML = '<span style="position:absolute;left:400px;top:60px;background:red">Right edge</span>';
          surface.hidden = true;
          target.append(surface);
          const stop = ctx.capture.registerTarget('export', {
            element: surface,
            prepare() { surface.hidden = false; },
            restore() { surface.hidden = true; }
          });
          return { unmount() { stop(); surface.remove(); } };
        } };
        """, to: stage)
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    do {
      let png = try #require(
        NSBitmapImageRep(data: try await SlopRenderer.capture(session: session, output: .exportPNG, provider: .webKit)))
      #expect(png.pixelsWide == 1040)
      let pdf = try #require(
        PDFDocument(data: try await SlopRenderer.capture(session: session, output: .pdf, provider: .webKit)))
      #expect(pdf.page(at: 0)?.bounds(for: .mediaBox).width == 520)
      #expect(pdf.string?.contains("Right edge") == true)
    } catch {
      try await session.close()
      throw error
    }
    try await session.close()
  }

  @Test(arguments: [0, 1, 40])
  @MainActor func exportIncludesAllSavedRowsRegardlessOfLocalView(count: Int) async throws {
    _ = NSApplication.shared
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    do {
      let rows = (0..<count).map { index -> [String: Any] in
        [
          "$id": "export-\(index)", "text": "Final task \(index)\nCafé 😀 multiline",
          "done": index % 2 == 0, "archived": false,
        ]
      }
      #expect(
        try await command(
          "batch", url: root,
          [
            "batch": [
              "intents": [
                ["type": "replace", "path": ["tasks"], "value": rows]
              ]
            ]
          ]
        ).ok)
      let expandedPNG = try await SlopRenderer.exportPNGData(session: session)
      _ = try await session.webView.evaluateJavaScript(
        "document.querySelector('[data-toggle-view]').click()")
      let pdf = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
      let page = try #require(pdf.page(at: 0))
      #expect(pdf.pageCount == 1)
      if count == 0 {
        #expect(pdf.string?.contains("Empty fixture") == true)
      } else {
        #expect(pdf.string?.contains("Final task \(count - 1)") == true)
        #expect(pdf.string?.contains("Café") == true)
      }
      let collapsedPNG = try await SlopRenderer.exportPNGData(session: session)
      #expect(collapsedPNG == expandedPNG)
      let png = try #require(NSBitmapImageRep(data: collapsedPNG))
      let bounds = page.bounds(for: .mediaBox)
      #expect(abs(CGFloat(png.pixelsWide) - bounds.width * 2) <= 2)
      // PNG and vector PDF share native layout and must include the long document.
      #expect(png.pixelsHigh > 0)
      if count == 40 {
        #expect(bounds.height > 1000)
        #expect(png.pixelsHigh > 2000)
      }
      #expect(
        try await session.webView.evaluateJavaScript(
          "document.querySelector('[data-toggle-view]').getAttribute('aria-pressed') === 'true'") as? Bool == true)
    } catch {
      try await session.close()
      throw error
    }
    try await session.close()
  }

  @Test(.nightly) @MainActor func captureDoesNotTouchTheEditorDuringResize() async throws {
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

  @Test(.nightly) @MainActor func pdfPreservesSurfaceColorAndSelectableText() async throws {
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
        // Sample the controlled solid surface away from text.
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
      // Native captures leave the editor mounted and interactive.
      #expect(
        try await session.webView.evaluateJavaScript("!!document.querySelector('[data-editor]')") as? Bool == true)
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
    let accepted = try await savedValue(root)
    _ = try await view.evaluateJavaScript("document.querySelector('[data-toggle-view]').click()")
    let pdf = try #require(PDFDocument(data: try await SlopRenderer.exportPDFData(session: session)))
    #expect(pdf.pageCount > 0)
    for row in rows { #expect(pdf.string?.contains(row["text"] as! String) == true) }
    #expect(
      try await view.evaluateJavaScript(
        "document.querySelector('[data-toggle-view]').getAttribute('aria-pressed') === 'true'") as? Bool == true)
    #expect(try await view.evaluateJavaScript(idle) as? Bool == true)
    #expect(try await savedValue(root) == accepted)
    try await session.close()
    // Background renders read the saved document in place, without owning or writing it.
    let saved = try Data(contentsOf: root)
    #expect(try await SlopRenderer.iconPNGData(url: root) != nil)
    #expect(try Data(contentsOf: root) == saved)
    #expect(try liveDiscovery(path: root.path) == nil)
  }

  // A window writes artwork from its saved state as it closes, when it changed the document or
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

  @Test(.nightly) @MainActor func longDocumentPreviewIsCappedWhileExportKeepsFullLength() async throws {
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
      for role in ["icon", "preview", "export"] {
        try Data("globalThis.__slopCapture = () => { throw new Error('capture test failure'); };".utf8)
          .write(to: stage.appendingPathComponent("assets/\(role).js"))
      }
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
      #expect(try await SlopRenderer.iconPNGData(url: source) != nil)
    }
    #expect(!FileManager.default.fileExists(atPath: try #require(sourceURL).path))
    #expect(try await savedValue(root)?["title"] as? String == "Later edit")
  }

  @Test @MainActor func nativeCapturesResolveSavedImageAttachments() async throws {
    let root = try contractFixture { stage in
      try Data(
        """
        globalThis.__slopCapture = json => {
          const request = JSON.parse(json), image = 'slop-attachment:' + request.snapshot.title;
          if (!request.resources[request.snapshot.title]) throw new Error('Missing saved image');
          return JSON.stringify({fonts:[],frames:[{
            kind:request.kind,width:request.width,height:request.height,
            html:'<html><body><p>Saved attachment</p><img src="'+image+'" width="100" height="100"></body></html>',
            css:'html,body{margin:0;background:white}p{font-family:sans-serif;font-size:20px;height:40px;margin:0}'
          }]});
        };
        """.utf8
      ).write(to: stage.appendingPathComponent("assets/export.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let blob = try Fixtures.png(width: 100, height: 100) {
      NSColor.blue.setFill()
      $0.fill()
    }
    _ = try await attach(blob, at: ["title"], url: root)
    let png = try #require(NSBitmapImageRep(data: await SlopRenderer.capture(url: root, output: .exportPNG)))
    let blue = try #require(png.colorAt(x: 50, y: 140)?.usingColorSpace(.sRGB))
    #expect(blue.blueComponent > 0.9 && blue.redComponent < 0.1)
    let pdf = try #require(PDFDocument(data: await SlopRenderer.capture(url: root, output: .pdf)))
    #expect(pdf.string?.contains("Saved attachment") == true)
    #expect(pdf.pageCount == 1)
  }

  @Test @MainActor func webKitProviderRemainsExplicitAndMissingNativeExportDoesNotFallback() async throws {
    let root = try contractFixture { stage in
      try Fixtures.updateInput(stage) { input in
        var declaration = input["declaration"] as! [String: Any]
        var views = declaration["views"] as! [String: Any]
        views["export"] = false
        declaration["views"] = views
        input["declaration"] = declaration
        var roles = input["roles"] as! [String: Any]
        roles.removeValue(forKey: "export")
        input["roles"] = roles
        input["resources"] = (input["resources"] as! [[String: Any]]).filter { $0["key"] as? String != "export.js" }
      }
      let script = """
        export default { mount(ctx, target) {
          const root = document.createElement('main'); root.dataset.slopRoot = '';
          root.textContent = 'Fallback: ' + ctx.document.current.title;
          target.append(root); return { unmount() { root.remove(); } };
        } };
        """
      try Data(script.utf8).write(to: stage.appendingPathComponent("assets/ui.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    // Production export refuses the absent role. The explicitly requested internal
    // WebKit provider can still capture the app for comparisons.
    let session = try await DocumentSession.open(url: root)
    session.load()
    try await session.waitUntilReady()
    #expect(try await command("batch", url: root, setTitle("Saved fallback")).ok)
    await #expect(throws: (any Error).self) {
      try await SlopRenderer.exportPDFData(session: session)
    }
    let pdf = try #require(
      PDFDocument(data: try await SlopRenderer.capture(session: session, output: .pdf, provider: .webKit)))
    #expect(pdf.string?.contains("Fallback: Saved fallback") == true)
    try await session.close()
  }

}

extension HostTests {
  @Test(.nightly) @MainActor func telemetryCountsCompletedExportsAndReportsFailuresWithoutDocumentValues() async throws
  {
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

extension HostTests {
  @Test @MainActor func previewAndExportUseTheirOwnSavedViews() async throws {
    let root = try contractFixture { stage in
      for (role, height, color, label) in [
        ("preview", 120, "#ff0000", "Preview only"), ("export", 900, "#0000ff", "Full export"),
      ] {
        let program = """
          globalThis.__slopCapture = json => {
            const request = JSON.parse(json);
            return JSON.stringify({fonts:[],frames:[{
              kind:request.kind,width:request.width,height:request.height,
              html:'<html><body>\(label)</body></html>',
              css:'html,body{margin:0;padding:0;height:\(height)px;background:\(color);font-family:sans-serif}'
            }]});
          };
          """
        try Data(program.utf8).write(to: stage.appendingPathComponent("assets/\(role).js"))
      }
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let preview = try #require(NSBitmapImageRep(data: await SlopRenderer.previewPNGData(url: root)))
    let exported = try #require(NSBitmapImageRep(data: await SlopRenderer.capture(url: root, output: .exportPNG)))
    #expect(preview.pixelsHigh == 240)
    #expect(exported.pixelsHigh == 1800)
    let previewColor = try #require(preview.colorAt(x: 100, y: 100)?.usingColorSpace(.sRGB))
    let exportColor = try #require(exported.colorAt(x: 100, y: 100)?.usingColorSpace(.sRGB))
    #expect(previewColor.redComponent > 0.9 && previewColor.blueComponent < 0.1)
    #expect(exportColor.blueComponent > 0.9 && exportColor.redComponent < 0.1)
    let pdf = try #require(PDFDocument(data: await SlopRenderer.capture(url: root, output: .pdf)))
    #expect(pdf.string?.contains("Full export") == true)
    #expect(pdf.string?.contains("Preview only") == false)
  }

  @Test @MainActor func absentExportHidesActionsAndRefusesDirectAndCLIExports() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      try Fixtures.updateInput(stage) { input in
        var declaration = input["declaration"] as! [String: Any]
        var views = declaration["views"] as! [String: Any]
        views["export"] = false
        declaration["views"] = views
        input["declaration"] = declaration
        var roles = input["roles"] as! [String: Any]
        roles.removeValue(forKey: "export")
        input["roles"] = roles
        input["resources"] = (input["resources"] as! [[String: Any]]).filter { $0["key"] as? String != "export.js" }
      }
    }
    let output = root.deletingLastPathComponent().appendingPathComponent(UUID().uuidString + ".pdf")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: output)
    }
    let controller = try await SlopDocumentWindowController.open(url: root)
    await controller.waitForPresentation()
    do {
      #expect(!controller.canExport)
      #expect(!controller.toolbarControls.canExport)
      #expect(!controller.sharePopover.canExport)
      #expect(controller.isAvailable(.shareSlop))
      for command in [SlopDocumentCommand.sharePNG, .sharePDF, .exportPNG, .exportPDF] {
        #expect(!controller.isAvailable(command))
        await #expect(throws: (any Error).self) { _ = try await controller.perform(command) }
      }
      await #expect(throws: (any Error).self) { _ = try await controller.prepareShare(format: .png) }
      await #expect(throws: (any Error).self) { try await SlopRenderer.exportPDFData(session: controller.session) }
      #expect(try await SlopRenderer.previewPNGData(session: controller.session).count > 0)
      #expect(try await SlopRenderer.iconPNGData(session: controller.session) != nil)
      let live = try await request([
        "method": "export", "documentPath": root.path, "format": "pdf", "output": output.path,
      ])
      #expect(live["ok"] as? Bool == false)
      #expect(!FileManager.default.fileExists(atPath: output.path))
      try await controller.finishClose()
    } catch {
      try? await controller.finishClose()
      throw error
    }
    let closed = try await request([
      "method": "export", "documentPath": root.path, "format": "pdf", "output": output.path,
    ])
    #expect(closed["ok"] as? Bool == false)
    #expect(!FileManager.default.fileExists(atPath: output.path))
  }
}

extension HostTests {
  @Test(.nightly) @MainActor func explicitWebKitProviderSelectsDedicatedPreviewTarget() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      try Fixtures.writeApp(
        """
        export default { mount(ctx, target) {
          const cleanup = [];
          for (const [role, height, color] of [['preview',120,'red'],['export',900,'blue']]) {
            const view = document.createElement('section');
            view.style.cssText = `width:320px;height:${height}px;background:${color}`;
            view.hidden = true; target.append(view);
            const stop = ctx.capture.registerTarget(role, {
              element:view, prepare(){ view.hidden=false; }, restore(){ view.hidden=true; }
            });
            cleanup.push(() => { stop(); view.remove(); });
          }
          return { unmount(){ for(const stop of cleanup) stop(); } };
        } };
        """, to: stage)
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let preview = try #require(
      NSBitmapImageRep(data: await SlopRenderer.capture(url: root, output: .previewPNG, provider: .webKit)))
    let exported = try #require(
      NSBitmapImageRep(data: await SlopRenderer.capture(url: root, output: .exportPNG, provider: .webKit)))
    #expect(preview.pixelsHigh == 240)
    #expect(exported.pixelsHigh == 1800)
    let red = try #require(preview.colorAt(x: 100, y: 100)?.usingColorSpace(.sRGB))
    let blue = try #require(exported.colorAt(x: 100, y: 100)?.usingColorSpace(.sRGB))
    #expect(red.redComponent > 0.9 && red.blueComponent < 0.1)
    #expect(blue.blueComponent > 0.9 && blue.redComponent < 0.1)
  }
}
