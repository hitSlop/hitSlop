import HitSlopCore
import HitSlopDocument
import SwiftUI
import UIKit
import WebKit

@MainActor final class PhoneDocuments: ObservableObject, DocumentSessionDelegate {
  @Published var documents: [URL] = []
  @Published var templates: [PhoneTemplate] = []
  @Published var localTitles: [URL: String] = [:]
  @Published var session: DocumentSession?
  @Published var busy = false
  @Published var saveStatus: DocumentSaveStatus = .saved
  var saveDescription: String {
    switch saveStatus {
    case .saved: "Saved locally"
    case .saving: "Saving…"
    case .failed: "Save failed"
    }
  }
  @Published var message: String?
  @Published var showImporter = false
  @Published var showTheme = false
  @Published var exported: URL?
  private var backgroundID: UIBackgroundTaskIdentifier = .invalid
  private var backgroundSave: Task<Void, Never>?
  private let folder = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("Documents", isDirectory: true)

  init() {
    SlopRegistry.sweep()
    do {
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      refresh()
    } catch { message = error.localizedDescription }
  }
  func loadTemplates() async {
    let root = Bundle.main.resourceURL?.appendingPathComponent("StarterTemplates", isDirectory: true)
    guard let root else { return }
    do {
      templates = try await SlopPreparation.run {
        try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
          .filter { $0.pathExtension == "slop" }.map { url in
            let summary = try SlopSummary(url: url, template: true)
            return PhoneTemplate(
              url: url, title: summary.metadata.title, description: summary.metadata.description,
              categories: summary.metadata.categories.map { $0.rawValue.capitalized })
          }
      }
    } catch { message = error.localizedDescription }
  }
  func create(_ template: PhoneTemplate, name: String) {
    perform {
      let entry = try await SlopLibrary.shared.create(template: template.url, title: name)
      try await self.show(URL(fileURLWithPath: entry.path))
    }
  }
  func delete(_ entry: SyncedSlop) {
    perform {
      if let session = self.session, session.file.url == URL(fileURLWithPath: entry.path) {
        let drafts = try await session.prepareClose()
        guard drafts.isEmpty else {
          await session.cancelClose()
          throw SlopFailure("Save your pending text before deleting.")
        }
        try await session.close()
        self.session = nil
      }
      try await SlopLibrary.shared.delete(entry)
    }
  }
  func libraryChanged() {
    guard let session, SlopLibrary.shared.entry(at: session.file.url)?.deleted == true, !busy else { return }
    perform {
      let drafts = try await session.prepareClose()
      guard drafts.isEmpty else {
        await session.cancelClose()
        throw SlopFailure("This slop was deleted on another device. Keep it open until your text is saved.")
      }
      try await session.close()
      self.session = nil
      self.message = "Deleted on another device. Any unsynced edits are kept as a local slop."
    }
  }
  func saveOnInactive() {
    guard let session, backgroundSave == nil else { return }
    Task { do { try await session.saveAccepted() } catch { message = error.localizedDescription } }
  }
  private func refresh() {
    documents = ((try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)) ?? [])
      .filter { $0.pathExtension == "slop" }.sorted { $0.lastPathComponent < $1.lastPathComponent }
    let paths = documents
    Task {
      localTitles = await Task.detached(priority: .utility) {
        Dictionary(
          uniqueKeysWithValues: paths.map { url in
            var name = url.deletingPathExtension().lastPathComponent
            if let dash = name.lastIndex(of: "-") {
              let suffix = name[name.index(after: dash)...]
              if suffix.count == 8 && suffix.allSatisfy({ $0.isHexDigit }) { name = String(name[..<dash]) }
            }
            return (url, name)
          })
      }.value
    }
  }
  private func perform(_ operation: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true
    Task {
      defer { busy = false }
      do { try await operation() } catch { message = error.localizedDescription }
    }
  }
  func importDocument(_ source: URL) {
    guard session == nil else {
      message = "Close your current document before importing another."
      return
    }
    perform {
      let target = self.folder.appendingPathComponent(
        source.deletingPathExtension().lastPathComponent + "-" + UUID().uuidString.prefix(8) + ".slop")
      let imported = try await SlopPreparation.run { () throws -> URL in
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        var outcome: Result<URL, Error>?
        var coordinationError: NSError?
        NSFileCoordinator().coordinate(readingItemAt: source, options: [], error: &coordinationError) { resolved in
          outcome = Result { try SlopFile.importCopy(from: resolved, to: target) }
        }
        if let coordinationError { throw coordinationError }
        guard let outcome else { throw SlopFailure("Files did not provide this document. Try downloading it first.") }
        return try outcome.get()
      }
      self.refresh()
      try await self.show(imported)
    }
  }
  func open(_ url: URL) { perform { try await self.show(url) } }
  private func show(_ url: URL) async throws {
    let opened = try await DocumentSession.open(url: url)
    opened.delegate = self
    session = opened
    saveStatus = .saved
    opened.load()
  }
  func close() {
    guard let session else { return }
    perform {
      let drafts = try await session.prepareClose()
      guard drafts.isEmpty else {
        await session.cancelClose()
        throw SlopFailure("Some text could not be saved. Keep this document open and copy that text before closing.")
      }
      do { _ = try await DocumentArtworkRenderer.refresh(session: session) } catch {
        NSLog("hitSlop artwork refresh failed: %@", error.localizedDescription)
      }
      try await session.close()
      self.session = nil
      self.refresh()
      await SlopLibrary.shared.refresh()
      Task { await SlopLibrary.shared.saveToCloud() }
    }
  }
  func undo(redo: Bool = false) {
    guard let session else { return }
    perform { try await session.undo(redo: redo) }
  }
  func save() {
    guard let session else { return }
    perform { try await session.saveAccepted() }
  }
  func export() {
    guard let session else { return }
    perform {
      let staging = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
      try FileManager.default.createDirectory(at: staging, withIntermediateDirectories: true)
      let output = staging.appendingPathComponent(session.file.url.lastPathComponent)
      do { self.exported = try await session.exportFile(to: output) } catch {
        try? FileManager.default.removeItem(at: staging)
        throw error
      }
    }
  }
  func finishExport() {
    if let exported { try? FileManager.default.removeItem(at: exported.deletingLastPathComponent()) }
    exported = nil
  }
  func saveInBackground() {
    guard let session, backgroundSave == nil else { return }
    backgroundID = UIApplication.shared.beginBackgroundTask(withName: "Save document") { [weak self] in
      Task { @MainActor in
        self?.backgroundSave?.cancel()
        self?.endBackground()
      }
    }
    backgroundSave = Task {
      defer {
        backgroundSave = nil
        endBackground()
      }
      do {
        try await session.pauseForBackground()
        await SlopLibrary.shared.saveToCloud()
      } catch {
        if !Task.isCancelled {
          message = "Your document is still open. Saving needs attention: \(error.localizedDescription)"
        }
      }
    }
  }
  private func endBackground() {
    guard backgroundID != .invalid else { return }
    UIApplication.shared.endBackgroundTask(backgroundID)
    backgroundID = .invalid
  }
  func pageSessionDidBecomeReady(_ session: DocumentSession) {}
  func pageSessionRecovered(_ session: DocumentSession) {}
  func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus) {
    self.saveStatus = saveStatus
    if case .failed(let failure) = saveStatus { message = failure.localizedDescription }
  }
  func pageSession(_ session: DocumentSession, storageFailure: SlopFailureContext) {
    message = "Document storage needs attention."
  }
  func pageSession(_ session: DocumentSession, themeChanged theme: SlopThemeState) {}
  func pageSession(_ session: DocumentSession, didReport issue: SlopPageIssue) { message = issue.message }
  func pageSession(_ session: DocumentSession, didFail error: Error) { message = error.localizedDescription }
  func pageSession(_ session: DocumentSession, didReplace webView: WKWebView) { objectWillChange.send() }
  func pageSession(_ session: DocumentSession, resizeContentTo size: CGSize) throws -> CGSize {
    session.webView.bounds.size
  }
}
