import AppKit
import Foundation
import HitSlopCore
import Testing
import HitSlopTestSupport
import WebKit
@testable import HitSlopDocument

@Suite(.serialized) struct DocumentFilePickerTests {
  @Test @MainActor func reloadAndCloseCancelThePendingPicker() async throws {
    _ = NSApplication.shared
    let root = try Fixtures.document()
    defer { try? FileManager.default.removeItem(at: root) }
    let engine = try await DocumentSession.open(url: root)
    engine.load()
    try await engine.waitUntilReady()
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = engine.webView
    defer { window.contentView = nil; window.orderOut(nil) }
    var completions = 0
    engine.filePicker = DocumentFilePicker { _, _, _, reply in { reply(nil) } }
    for closing in [false, true] {
      engine.filePicker.present(in: window, multiple: false, directories: false) { urls in
        #expect(urls == nil)
        completions += 1
      }
      #expect(engine.filePicker.hasPendingSelection)
      if closing { try await engine.close() } else { try await engine.reloadInterface() }
      #expect(!engine.filePicker.hasPendingSelection)
    }
    #expect(completions == 2)
  }

  // The HTML upload boundary is platform behavior, independent of a slop's importer.
  @Test(.timeLimit(.minutes(1))) @MainActor func fileInputDeliversBytesAndRejectsSelectionDuringCapture() async throws {
    _ = NSApplication.shared
    let root = try Fixtures.document()
    let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".txt")
    try Data("Selected 🦊 bytes".utf8).write(to: file)
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: file) }
    let engine = try await DocumentSession.open(url: root)
    engine.load()
    try await engine.waitUntilReady()
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = engine.webView
    window.orderFront(nil)
    defer { window.contentView = nil; window.orderOut(nil) }
    var presentations = 0
    engine.filePicker = DocumentFilePicker { parent, multiple, directories, reply in
      #expect(parent === window && !multiple && !directories)
      presentations += 1
      reply([file])
      return {}
    }
    for mode in ["normal", "capture", "disabled"] {
      engine.capturing = mode == "capture"
      engine.allowsFileSelection = mode != "disabled"
      let result = try await engine.webView.callAsyncJavaScript("""
        const input = document.createElement('input'); input.type = 'file'; document.body.append(input);
        try {
          return await new Promise(resolve => {
            input.oncancel = () => resolve(null);
            input.onchange = async () => resolve(await input.files[0].text());
            input.click();
          });
        } finally { input.remove(); }
        """, arguments: [:], in: nil, contentWorld: .page)
      #expect(result as? String == (mode == "normal" ? "Selected 🦊 bytes" : nil))
    }
    #expect(presentations == 1)
    engine.capturing = false
    engine.allowsFileSelection = true
    try await engine.close()
  }

  @Test @MainActor func pickerCompletesCancellationOnceAndRejectsConcurrentRequests() async throws {
    _ = NSApplication.shared
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
    window.orderFront(nil)
    defer { window.orderOut(nil) }
    var completions = 0, rejected = 0, dismissals = 0
    var selected: (([URL]?) -> Void)?
    let picker = DocumentFilePicker { parent, multiple, directories, reply in
      #expect(parent === window)
      #expect(multiple && !directories)
      selected = reply
      return { dismissals += 1; reply(nil) }
    }
    picker.present(in: window, multiple: true, directories: false) { urls in
      #expect(urls == nil); completions += 1
    }
    #expect(picker.hasPendingSelection)
    picker.present(in: window, multiple: false, directories: false) { urls in
      #expect(urls == nil); rejected += 1
    }
    #expect(rejected == 1)
    picker.cancel()
    picker.cancel()
    selected?([URL(fileURLWithPath: "/tmp/late-selection.wsz")])
    #expect(completions == 1)
    #expect(dismissals == 1)
  }

}
