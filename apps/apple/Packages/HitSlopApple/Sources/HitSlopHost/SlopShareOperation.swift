import AppKit
import HitSlopCore

/// Owns a temporary document until the picker is cancelled or the selected service
/// finishes. Active services outlive the window that started them.
// AppKit invokes picker callbacks on the UI thread; unlike the service callbacks,
// the SDK's picker protocol does not yet carry main-actor annotations.
@MainActor
final class SlopShareOperation: NSObject, @preconcurrency NSSharingServicePickerDelegate, NSSharingServiceDelegate {
  private static var active: [UUID: SlopShareOperation] = [:]
  private let id = UUID()
  let directory: URL
  let file: URL
  private(set) var picker: NSSharingServicePicker?
  private var service: NSSharingService?
  private var finished = false
  private let telemetry: SlopTelemetry
  private var pickerDismissed: (() -> Void)?

  private init(filename: String, telemetry: SlopTelemetry) throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("hitSlop Share \(UUID().uuidString)")
    file = directory.appendingPathComponent(filename)
    self.telemetry = telemetry
    super.init()
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  }

  static func prepare(
    filename: String, telemetry: SlopTelemetry = .disabled,
    copy: @MainActor (URL) async throws -> Void
  ) async throws -> SlopShareOperation {
    let operation = try SlopShareOperation(filename: filename, telemetry: telemetry)
    do {
      try await copy(operation.file)
      try Task.checkCancellation()
      return operation
    } catch {
      operation.finish()
      throw error
    }
  }

  func present(
    in view: NSView?,
    pickerDismissed: @escaping () -> Void = {},
    show: (NSSharingServicePicker, NSView) -> Void = {
      $0.show(relativeTo: $1.bounds, of: $1, preferredEdge: .minY)
    }
  ) throws {
    guard let view, view.window != nil else {
      finish()
      throw SlopFailure("The document window is no longer available to share")
    }
    let picker = NSSharingServicePicker(items: [file])
    self.picker = picker
    picker.delegate = self
    self.pickerDismissed = pickerDismissed
    Self.active[id] = self
    show(picker, view)
  }

  func sharingServicePicker(
    _ sharingServicePicker: NSSharingServicePicker,
    delegateFor sharingService: NSSharingService
  ) -> (any NSSharingServiceDelegate)? {
    service = sharingService
    return self
  }

  func sharingServicePicker(
    _ sharingServicePicker: NSSharingServicePicker,
    didChoose service: NSSharingService?
  ) {
    releasePicker()
    guard let service else {
      telemetry.send(.breadcrumb(.share, .cancelled))
      finish()
      return
    }
    self.service = service
  }

  func sharingService(_ sharingService: NSSharingService, didShareItems items: [Any]) {
    telemetry.send(.breadcrumb(.share, .completed))
    finish()
  }

  func sharingService(_ sharingService: NSSharingService, didFailToShareItems items: [Any], error: Error) {
    telemetry.failure(.share, error: error)
    finish()
  }

  private func finish() {
    guard !finished else { return }
    finished = true
    releasePicker()
    picker?.delegate = nil
    picker = nil
    service?.delegate = nil
    service = nil
    try? FileManager.default.removeItem(at: directory)
    Self.active[id] = nil
  }

  private func releasePicker() {
    let dismissed = pickerDismissed
    pickerDismissed = nil
    dismissed?()
  }

  deinit { try? FileManager.default.removeItem(at: directory) }
}
