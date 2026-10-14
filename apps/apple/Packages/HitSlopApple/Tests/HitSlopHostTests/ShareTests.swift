import AppKit
import HitSlopCore
import HitSlopTestSupport
import PDFKit
import Testing

@testable import HitSlopHost

extension HostTests {
  @Test(arguments: [false, true], ["slop", "pdf", "png"]) @MainActor
  func shareStagingLivesUntilTheServiceFinishes(failed: Bool, fileExtension: String) async throws {
    var operation: SlopShareOperation? = try await SlopShareOperation.prepare(filename: "copy.\(fileExtension)") {
      try Data("Shared bytes".utf8).write(to: $0)
    }
    let file = try #require(operation?.file)
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
      styleMask: [.titled], backing: .buffered, defer: false)
    window.moveOffScreen()
    window.isReleasedWhenClosed = false
    var pickerDismissals = 0
    try operation?.present(in: window.contentView, pickerDismissed: { pickerDismissals += 1 }, show: { _, _ in })
    let picker = try #require(operation?.picker)
    let service = NSSharingService(
      title: "Test", image: NSImage(size: NSSize(width: 1, height: 1)),
      alternateImage: nil, handler: {})
    service.delegate = picker.delegate?.sharingServicePicker?(picker, delegateFor: service)
    picker.delegate?.sharingServicePicker?(picker, didChoose: service)
    #expect(pickerDismissals == 1)
    weak var retained = operation
    operation = nil
    window.close()
    #expect(retained != nil)
    #expect(FileManager.default.fileExists(atPath: file.path))
    if failed {
      service.delegate?.sharingService?(service, didFailToShareItems: [file], error: SlopFailure("Test failure"))
    } else {
      service.delegate?.sharingService?(service, didShareItems: [file])
    }
    #expect(!FileManager.default.fileExists(atPath: file.deletingLastPathComponent().path))
    #expect(retained == nil)
    #expect(pickerDismissals == 1)
  }

  @Test @MainActor func cancelledAndUnpresentableSharesRemoveStaging() async throws {
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
      styleMask: [.titled], backing: .buffered, defer: false)
    window.moveOffScreen()
    window.isReleasedWhenClosed = false
    defer { window.close() }
    for present in [false, true] {
      let operation = try await SlopShareOperation.prepare(filename: "copy.slop") {
        try Data("Shared bytes".utf8).write(to: $0)
      }
      if present {
        try operation.present(in: window.contentView, show: { _, _ in })
        let picker = try #require(operation.picker)
        picker.delegate?.sharingServicePicker?(picker, didChoose: nil)
      } else {
        #expect(throws: (any Error).self) { try operation.present(in: nil) }
      }
      #expect(!FileManager.default.fileExists(atPath: operation.directory.path))
    }
    var staging: URL?
    do {
      _ = try await SlopShareOperation.prepare(filename: "copy.slop") {
        staging = $0.deletingLastPathComponent()
        throw SlopFailure("Copy failed")
      }
      Issue.record("Expected the copy failure")
    } catch {}
    #expect(!FileManager.default.fileExists(atPath: try #require(staging).path))
  }

  @Test(arguments: ["pdf", "png", "slop"]) @MainActor
  func sharedFormatsIncludePendingEdits(fileExtension: String) async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    await controller.waitForPresentation()
    do {
      // Leave text in the page's input; sharing must drain it before taking the snapshot.
      _ = try await controller.session.webView.evaluateJavaScript(
        "const input = document.querySelector('#draft'); input.value = 'Shared pending edit'; input.dispatchEvent(new Event('input', {bubbles:true})); true"
      )
      let format: ExportFormat? = fileExtension == "pdf" ? .pdf : fileExtension == "png" ? .png : nil
      let operation = try await controller.prepareShare(format: format)
      #expect(operation.file.lastPathComponent == root.deletingPathExtension().lastPathComponent + "." + fileExtension)
      #expect(try await savedValue(root)?["title"] as? String == "Shared pending edit")
      if fileExtension == "pdf" {
        #expect(PDFDocument(url: operation.file)?.string?.contains("Shared pending edit") == true)
      } else if fileExtension == "png" {
        let image = try #require(NSBitmapImageRep(data: Data(contentsOf: operation.file)))
        #expect(image.pixelsWide > 0 && image.pixelsHigh > 0)
      } else {
        #expect(try await savedValue(operation.file)?["title"] as? String == "Shared pending edit")
        let copy = try await SlopDocumentWindowController.open(url: operation.file)
        try await copy.session.waitUntilReady()
        #expect(try await command("batch", url: operation.file, setTitle("Independent copy")).ok)
        #expect(try await savedValue(root)?["title"] as? String == "Shared pending edit")
        try await copy.finishClose()
      }
      try await controller.finishClose()
    } catch {
      try? await controller.finishClose()
      throw error
    }
  }

  @Test @MainActor func shareChooserDoesNotCreateAnExportAndKeepsToolbarVisible() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    controller.showWindow(nil)
    await controller.waitForPresentation()
    SlopToolbarPointerSampler.shared.remove(controller.toolbar)
    func stagedShares() throws -> Set<String> {
      Set(
        try FileManager.default.contentsOfDirectory(atPath: FileManager.default.temporaryDirectory.path)
          .filter { $0.hasPrefix("hitSlop Share ") })
    }
    do {
      let before = try stagedShares()
      _ = try await controller.perform(.share)
      #expect(controller.sharePopover.isShown)
      #expect(try stagedShares() == before)
      let outside = NSPoint(x: -10000, y: -10000)
      controller.toolbar.refresh(point: outside, front: 0, now: 1)
      controller.toolbar.refresh(point: outside, front: 0, now: 10)
      #expect(controller.toolbar.isVisible)
      _ = try await controller.perform(.share)
      #expect(!controller.sharePopover.isShown)
      _ = try await controller.perform(.share)
      #expect(controller.sharePopover.isShown)
      controller.windowWillMiniaturize(
        Notification(name: NSWindow.willMiniaturizeNotification, object: controller.window))
      #expect(!controller.sharePopover.isShown)
      #expect(!controller.toolbar.isVisible)
      try await controller.finishClose()
    } catch {
      try? await controller.finishClose()
      throw error
    }
  }
}
