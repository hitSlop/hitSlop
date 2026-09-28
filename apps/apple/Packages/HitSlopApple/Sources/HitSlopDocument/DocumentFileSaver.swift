import AppKit
import CoreServices
import UniformTypeIdentifiers
@preconcurrency import WebKit

/// Saves files a slop offers with `<a download href="blob:…">`. WebKit writes into a
/// private staging file; the chosen destination is replaced only after a complete download.
@MainActor final class DocumentFileSaver: NSObject, WKDownloadDelegate {
  typealias Presenter = @MainActor (NSWindow, String, @escaping (URL?) -> Void) -> (() -> Void)
  nonisolated static let maximumBytes: Int64 = 100 * 1024 * 1024

  private final class Transfer {
    let download: WKDownload
    var staging: URL?
    var destination: URL?
    var completion: ((URL?) -> Void)?
    var dismiss: (() -> Void)?
    var observation: NSKeyValueObservation?
    init(_ download: WKDownload) { self.download = download }
  }
  private let presenter: Presenter
  private var transfers: [ObjectIdentifier: Transfer] = [:]
  var window: () -> NSWindow? = { nil }
  var onSaved: ((URL) -> Void)?
  var onFailed: ((String) -> Void)?
  var hasPendingSave: Bool { transfers.values.contains { $0.completion != nil } }
  var hasActiveTransfers: Bool { !transfers.isEmpty }

  init(presenter: @escaping Presenter = DocumentFileSaver.presentSheet) {
    self.presenter = presenter
  }

  func begin(_ download: WKDownload) {
    let transfer = Transfer(download)
    transfers[ObjectIdentifier(download)] = transfer
    download.delegate = self
    transfer.observation = download.progress.observe(\.completedUnitCount, options: [.new]) {
      [weak self, weak download] progress, _ in
      guard progress.completedUnitCount > Self.maximumBytes else { return }
      Task { @MainActor in
        guard let self, let download,
          self.transfers[ObjectIdentifier(download)] != nil else { return }
        self.stop(download)
        self.onFailed?("Download exceeds the 100 MiB limit.")
      }
    }
  }

  func cancel() {
    for transfer in Array(transfers.values) { stop(transfer.download) }
  }

  private func stop(_ download: WKDownload) {
    guard let transfer = transfers.removeValue(forKey: ObjectIdentifier(download)) else { return }
    transfer.observation = nil
    let completion = transfer.completion
    transfer.completion = nil
    transfer.dismiss?()
    transfer.dismiss = nil
    completion?(nil)
    download.cancel { _ in
      if let staging = transfer.staging { try? FileManager.default.removeItem(at: staging) }
    }
  }

  func download(
    _ download: WKDownload, decideDestinationUsing response: URLResponse,
    suggestedFilename: String, completionHandler: @escaping @MainActor (URL?) -> Void
  ) {
    guard let transfer = transfers[ObjectIdentifier(download)], !hasPendingSave,
      let window = window(), window.isVisible, window.attachedSheet == nil
    else { completionHandler(nil); stop(download); return }
    guard response.expectedContentLength <= Self.maximumBytes else {
      completionHandler(nil)
      stop(download)
      onFailed?("Download exceeds the 100 MiB limit.")
      return
    }
    transfer.completion = completionHandler
    let dismiss = presenter(window, Self.filename(suggestedFilename)) { [weak self, weak transfer] target in
      guard let self, let transfer,
        self.transfers[ObjectIdentifier(download)] === transfer,
        let completion = transfer.completion else { return }
      transfer.completion = nil
      transfer.dismiss = nil
      guard let target else { completion(nil); stop(download); return }
      let staging = FileManager.default.temporaryDirectory
        .appendingPathComponent("hitslop-download-" + UUID().uuidString)
      transfer.staging = staging
      transfer.destination = target
      completion(staging)
    }
    // A presenter may complete synchronously (including injected test presenters).
    if transfer.completion != nil { transfer.dismiss = dismiss }
  }

  func downloadDidFinish(_ download: WKDownload) {
    guard let transfer = transfers.removeValue(forKey: ObjectIdentifier(download)),
      let staging = transfer.staging, let destination = transfer.destination else { return }
    transfer.observation = nil
    defer { try? FileManager.default.removeItem(at: staging) }
    do {
      try Self.install(staging, at: destination)
      onSaved?(destination)
    } catch { onFailed?(error.localizedDescription) }
  }

  /// Copy to the destination volume before the final atomic replacement.
  static func install(_ staging: URL, at destination: URL, limit: Int64 = maximumBytes,
    quarantine: (URL) throws -> Void = applyQuarantine
  ) throws {
    let manager = FileManager.default
    let attributes = try manager.attributesOfItem(atPath: staging.path)
    guard let size = attributes[.size] as? NSNumber, size.int64Value <= limit else {
      throw NSError(domain: "hitSlop.Download", code: 1,
        userInfo: [NSLocalizedDescriptionKey: "Download exceeds the 100 MiB limit."])
    }
    let replacement = try manager.url(for: .itemReplacementDirectory, in: .userDomainMask,
      appropriateFor: destination, create: true)
    defer { try? manager.removeItem(at: replacement) }
    let ready = replacement.appendingPathComponent("download")
    try manager.copyItem(at: staging, to: ready)
    // Authored bytes must be quarantined before they become visible at the
    // chosen destination. A metadata failure must preserve any existing file.
    try quarantine(ready)
    if manager.fileExists(atPath: destination.path) {
      _ = try manager.replaceItemAt(destination, withItemAt: ready, options: .usingNewMetadataOnly)
    } else {
      try manager.moveItem(at: ready, to: destination)
    }
  }

  nonisolated private static func applyQuarantine(_ file: URL) throws {
    var file = file
    var values = URLResourceValues()
    values.quarantineProperties = [
      kLSQuarantineAgentNameKey as String: "hitSlop",
      kLSQuarantineTypeKey as String: kLSQuarantineTypeOtherDownload,
      kLSQuarantineTimeStampKey as String: Date(),
    ]
    try file.setResourceValues(values)
  }

  func download(_ download: WKDownload, didFailWithError error: Error, resumeData: Data?) {
    guard transfers[ObjectIdentifier(download)] != nil else { return }
    stop(download)
    if (error as NSError).domain != NSURLErrorDomain || (error as NSError).code != NSURLErrorCancelled {
      onFailed?(error.localizedDescription)
    }
  }

  /// Suggested names come from authored code; keep only a plain file name.
  nonisolated static func filename(_ suggested: String) -> String {
    let name = (suggested as NSString).lastPathComponent
      .components(separatedBy: CharacterSet(charactersIn: ":/\\").union(.controlCharacters))
      .joined(separator: "-")
      .trimmingCharacters(in: .whitespacesAndNewlines.union(CharacterSet(charactersIn: ".")))
    return name.isEmpty ? "Download" : String(name.prefix(200))
  }

  private static func presentSheet(in window: NSWindow, filename: String,
    completion: @escaping (URL?) -> Void
  ) -> (() -> Void) {
    let panel = NSSavePanel()
    panel.nameFieldStringValue = filename
    let ext = (filename as NSString).pathExtension
    if !ext.isEmpty, let type = UTType(filenameExtension: ext) { panel.allowedContentTypes = [type] }
    panel.canCreateDirectories = true
    panel.beginSheetModal(for: window) { response in
      completion(response == .OK ? panel.url : nil)
    }
    return { panel.cancel(nil) }
  }
}
