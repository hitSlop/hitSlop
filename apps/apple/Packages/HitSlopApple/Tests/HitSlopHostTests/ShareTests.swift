import AppKit
import HitSlopCore
import Testing

@testable import HitSlopHost

extension HostTests {
  @Test(arguments: [false, true]) @MainActor
  func shareStagingLivesUntilTheServiceFinishes(failed: Bool) async throws {
    var operation: SlopShareOperation? = try await SlopShareOperation.prepare(filename: "copy.slop") {
      try Data("Shared bytes".utf8).write(to: $0)
    }
    let file = try #require(operation?.file)
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 100, height: 100),
      styleMask: [.titled], backing: .buffered, defer: false)
    window.moveOffScreen()
    window.isReleasedWhenClosed = false
    try operation?.present(in: window.contentView, show: { _, _ in })
    let picker = try #require(operation?.picker)
    let service = NSSharingService(
      title: "Test", image: NSImage(size: NSSize(width: 1, height: 1)),
      alternateImage: nil, handler: {})
    service.delegate = picker.delegate?.sharingServicePicker?(picker, delegateFor: service)
    picker.delegate?.sharingServicePicker?(picker, didChoose: service)
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
}
