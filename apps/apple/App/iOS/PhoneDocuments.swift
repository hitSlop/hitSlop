import SwiftUI
import TinysCore
import TinysCoreBinding
import TinysDocument
import UIKit
import WebKit

struct PhoneRecent: Identifiable, Sendable {
  let url: URL
  let title: String
  let entry: SyncedTiny?
  var id: String { url.standardizedFileURL.path }
}

@MainActor final class PhoneDocuments: ObservableObject, DocumentSessionDelegate {
  @Published var documents: [URL] = []
  @Published var templates: [PhoneTemplate] = []
  @Published var localTitles: [URL: String] = [:]
  @Published private var fileDates: [URL: Date] = [:]
  @Published private var openedAt: [String: Double] = [:]
  @Published var session: DocumentSession?
  @Published var opening = false
  @Published var busy = false
  @Published var saveStatus: DocumentSaveStatus = .saved
  var saveDescription: String {
    switch saveStatus {
    case .saved: "Saved locally"
    case .saving: "Saving…"
    case .failed: "Save failed"
    }
  }
  @Published var successFeedback = 0
  @Published var deletionFeedback = 0
  /// A light tap for a deliberate host action: opening a tiny, a menu choice.
  @Published var tapFeedback = 0
  func tapped() { tapFeedback += 1 }
  private var manualSync = false
  @Published var message: String?
  @Published var showImporter = false
  @Published var showTheme = false
  @Published var exported: URL?
  private var pendingLibraryID: String?
  private(set) lazy var widgets = PhoneWidgetPublisher { [weak self] in self?.session }
  private var backgroundID: UIBackgroundTaskIdentifier = .invalid
  private var backgroundSave: Task<Void, Never>?
  private var refreshTask: Task<Void, Never>?
  private static let recentsKey = "phone.recents.openedAt"
  private let folder = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("Documents", isDirectory: true)

  init() {
    openedAt = UserDefaults.standard.dictionary(forKey: Self.recentsKey) as? [String: Double] ?? [:]
    _ = widgets
    TinyRegistry.sweep()
    do {
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      refresh()
    } catch { message = error.localizedDescription }
  }
  func loadTemplates() async {
    let root = Bundle.main.resourceURL?.appendingPathComponent("StarterTemplates", isDirectory: true)
    guard let root else { return }
    do {
      templates = try await TinyPreparation.run {
        try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
          .filter { $0.pathExtension == "tiny" }.map { url in
            let summary = try TinySummary(url: url, template: true)
            return PhoneTemplate(
              url: url, title: summary.metadata.title, description: summary.metadata.description,
              categories: summary.metadata.categories.map { $0.rawValue.capitalized })
          }
      }
    } catch { message = error.localizedDescription }
  }
  func create(_ template: PhoneTemplate, name: String) {
    perform {
      let entry = try await TinyLibrary.shared.create(template: template.url, title: name)
      try await self.show(URL(fileURLWithPath: entry.path))
      self.successFeedback += 1
    }
  }
  func recents(in entries: [SyncedTiny]) -> [PhoneRecent] {
    let synced = entries.map { PhoneRecent(url: URL(fileURLWithPath: $0.path), title: $0.title, entry: $0) }
    let managed = Set(synced.map(\.id))
    let local = documents.compactMap { url -> PhoneRecent? in
      guard !managed.contains(url.standardizedFileURL.path) else { return nil }
      return PhoneRecent(url: url, title: localTitles[url] ?? url.deletingPathExtension().lastPathComponent, entry: nil)
    }
    func activity(_ recent: PhoneRecent) -> Date {
      max(
        fileDates[recent.url] ?? .distantPast,
        openedAt[recent.id].map(Date.init(timeIntervalSince1970:)) ?? .distantPast)
    }
    return (synced + local).sorted {
      let first = activity($0)
      let second = activity($1)
      if first != second { return first > second }
      let titleOrder = $0.title.localizedStandardCompare($1.title)
      return titleOrder == .orderedSame ? $0.id < $1.id : titleOrder == .orderedAscending
    }
  }
  func delete(_ recent: PhoneRecent) {
    perform {
      if let session = self.session, session.file.url == recent.url {
        let drafts = try await session.prepareClose()
        guard drafts.isEmpty else {
          await session.cancelClose()
          throw TinyFailure("Save your pending text before deleting.")
        }
        try await session.close()
        self.session = nil
        self.opening = false
      }
      if let entry = recent.entry {
        try await TinyLibrary.shared.delete(entry)
      } else {
        try await TinyPreparation.run { try TinyFile.delete(recent.url) }
      }
      self.openedAt.removeValue(forKey: recent.id)
      UserDefaults.standard.set(self.openedAt, forKey: Self.recentsKey)
      self.refresh()
      self.deletionFeedback += 1
    }
  }
  func removeDownload(_ entry: SyncedTiny) {
    perform {
      try await TinyLibrary.shared.removeDownload(entry)
      self.refresh()
      self.deletionFeedback += 1
    }
  }
  /// Only explicit sync requests produce feedback; polling and background sync do not.
  func syncNow() async {
    guard !manualSync else { return }
    manualSync = true
    defer { manualSync = false }
    let library = TinyLibrary.shared
    let previousSuccess = library.lastSuccess
    await library.syncNow()
    if !library.isReady || library.lastError != nil || !library.issues.isEmpty {
      return
    } else if let lastSuccess = library.lastSuccess, lastSuccess != previousSuccess, library.pending == 0 {
      successFeedback += 1
    }
  }
  func libraryChanged() {
    refresh()
    if TinyLibrary.shared.hasLoaded, let id = pendingLibraryID {
      pendingLibraryID = nil
      open(libraryID: id)
    }
    guard let session, TinyLibrary.shared.entry(at: session.file.url)?.deleted == true, !busy else { return }
    perform {
      let drafts = try await session.prepareClose()
      guard drafts.isEmpty else {
        await session.cancelClose()
        throw TinyFailure("This tiny was deleted on another device. Keep it open until your text is saved.")
      }
      try await session.close()
      self.session = nil
      self.opening = false
      self.message = "Deleted on another device. Any unsynced edits are kept as a local tiny."
    }
  }
  func saveOnInactive() {
    guard let session, backgroundSave == nil else { return }
    Task { do { try await session.saveAccepted() } catch { message = error.localizedDescription } }
  }
  private func refresh() {
    let shared = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("tinys/SharedDocuments", isDirectory: true)
    let local = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    documents = [folder, shared, local].flatMap {
      (try? FileManager.default.contentsOfDirectory(at: $0, includingPropertiesForKeys: nil)) ?? []
    }
    .filter { $0.pathExtension == "tiny" }.sorted { $0.lastPathComponent < $1.lastPathComponent }
    let paths = Set(documents + TinyLibrary.shared.visibleEntries.map { URL(fileURLWithPath: $0.path) })
    refreshTask?.cancel()
    refreshTask = Task {
      let metadata = await Task.detached(priority: .utility) {
        var titles: [URL: String] = [:]
        var dates: [URL: Date] = [:]
        for url in paths {
          var name = url.deletingPathExtension().lastPathComponent
          if let dash = name.lastIndex(of: "-") {
            let suffix = name[name.index(after: dash)...]
            if suffix.count == 8 && suffix.allSatisfy({ $0.isHexDigit }) { name = String(name[..<dash]) }
          }
          titles[url] = name
          let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .creationDateKey])
          dates[url] = max(values?.contentModificationDate ?? .distantPast, values?.creationDate ?? .distantPast)
        }
        return (titles, dates)
      }.value
      guard !Task.isCancelled else { return }
      localTitles = metadata.0
      fileDates = metadata.1
    }
  }
  private func perform(
    completion: @escaping @MainActor (Bool) -> Void = { _ in },
    _ operation: @escaping @MainActor () async throws -> Void
  ) {
    guard !busy else {
      completion(false)
      return
    }
    busy = true
    Task {
      let succeeded: Bool
      do {
        try await operation()
        succeeded = true
      } catch {
        message = error.localizedDescription
        succeeded = false
      }
      busy = false
      completion(succeeded)
    }
  }
  func importDocument(_ source: URL) {
    guard session == nil else {
      message = "Close your current document before importing another."
      return
    }
    perform {
      let target = self.folder.appendingPathComponent(
        source.deletingPathExtension().lastPathComponent + "-" + UUID().uuidString.prefix(8) + ".tiny")
      let imported = try await TinyPreparation.run { () throws -> URL in
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        var outcome: Result<URL, Error>?
        var coordinationError: NSError?
        NSFileCoordinator().coordinate(readingItemAt: source, options: [], error: &coordinationError) { resolved in
          outcome = Result { try TinyFile.importCopy(from: resolved, to: target) }
        }
        if let coordinationError { throw coordinationError }
        guard let outcome else { throw TinyFailure("Files did not provide this document. Try downloading it first.") }
        return try outcome.get()
      }
      self.refresh()
      try await self.show(imported)
    }
  }
  func open(_ url: URL) {
    tapped()
    perform { try await self.show(url) }
  }
  func open(libraryID: String) {
    guard TinyLibrary.shared.hasLoaded else {
      pendingLibraryID = libraryID
      TinyLibrary.shared.start()
      return
    }
    // Resolve before closing, so a stale widget never costs the open document.
    guard let entry = TinyLibrary.shared.visibleEntries.first(where: { $0.id == libraryID }) else {
      message = "This document is no longer in your library."
      return
    }
    let url = URL(fileURLWithPath: entry.path)
    guard session?.file.url != url else { return }
    close { [self] closed in
      // Sync may have removed it while the current document closed.
      guard closed, let entry = TinyLibrary.shared.visibleEntries.first(where: { $0.id == libraryID }) else { return }
      open(URL(fileURLWithPath: entry.path))
    }
  }
  func join(_ link: String) async throws {
    if let session {
      let incoming = try await TinyPreparation.run { try sharingDocument(link: link) }
      guard incoming == session.documentID else {
        throw TinyFailure("Close your current document before joining another.")
      }
    }
    let url = try await DocumentSharing.join(link, candidates: documents)
    refresh()
    if session?.file.url != url { try await show(url) }
    successFeedback += 1
  }
  private func show(_ url: URL) async throws {
    opening = true
    do {
      if let entry = TinyLibrary.shared.entry(at: url), !entry.downloaded {
        try await TinyLibrary.shared.download(entry)
      }
      let opened = try await DocumentSession.open(url: url)
      opened.delegate = self
      session = opened
      openedAt[url.standardizedFileURL.path] = Date().timeIntervalSince1970
      UserDefaults.standard.set(openedAt, forKey: Self.recentsKey)
      saveStatus = .saved
      opened.load()
    } catch {
      opening = false
      throw error
    }
  }
  func close(completion: @escaping @MainActor (Bool) -> Void = { _ in }) {
    guard let session else {
      completion(true)
      return
    }
    perform(completion: completion) {
      let drafts = try await session.prepareClose()
      guard drafts.isEmpty else {
        await session.cancelClose()
        throw TinyFailure("Some text could not be saved. Keep this document open and copy that text before closing.")
      }
      await self.widgets.closing(session)
      do { _ = try await DocumentArtworkRenderer.refresh(session: session) } catch {
        NSLog("tinys artwork refresh failed: %@", error.localizedDescription)
      }
      try await session.close()
      self.session = nil
      self.opening = false
      self.refresh()
      await TinyLibrary.shared.refresh()
      Task { await TinyLibrary.shared.saveToCloud() }
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
  /// Moves a full shared document to an unshared copy of its accepted state, with accepted
  /// page edits. The owner publishes the copy at a new path and releases the original
  /// without saving it; until that succeeds the document stays open with every edit, and
  /// other participants keep it.
  func saveUnsharedCopy() {
    guard let session else { return }
    perform {
      let directory = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
      let name = session.file.url.deletingPathExtension().lastPathComponent
      let target = directory.appendingPathComponent("\(name) copy \(UUID().uuidString.prefix(8)).tiny")
      let copied = try await session.handOff(to: target)
      self.session = nil
      self.opening = false
      self.refresh()
      self.successFeedback += 1
      await TinyLibrary.shared.refresh()
      Task { await TinyLibrary.shared.saveToCloud() }
      try await self.show(copied)
    }
  }
  func finishExport(completed: Bool = false) {
    if completed { successFeedback += 1 }
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
      widgets.beginBackgroundRefresh()
      defer {
        widgets.endBackgroundRefresh()
        backgroundSave = nil
        endBackground()
      }
      do {
        try await session.pauseForBackground()
        try Task.checkCancellation()
        await TinyLibrary.shared.saveToCloud()
        try Task.checkCancellation()
        await widgets.refreshPlaced()
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
  func pageSessionDidBecomeReady(_ session: DocumentSession) {
    if self.session === session { opening = false }
    publishSafeArea(session)
  }
  func pageSessionRecovered(_ session: DocumentSession) {
    if self.session === session { opening = false }
    publishSafeArea(session)
  }
  /// The edge-to-edge page pads itself by the status bar and home indicator heights. A ready
  /// page has its shell entry point; the key window knows the insets even before the page's
  /// view is attached, and WebKit reports none for a page that ignores the safe area.
  private func publishSafeArea(_ session: DocumentSession) {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    guard
      let window = (scenes.first { $0.activationState == .foregroundActive } ?? scenes.first)?.keyWindow,
      let webView = session.currentWebView
    else { return }
    let (top, bottom) = (Int(window.safeAreaInsets.top.rounded()), Int(window.safeAreaInsets.bottom.rounded()))
    webView.evaluateJavaScript("globalThis.__tiny?.safeArea?.(\(top), \(bottom))", in: nil, in: .page) { _ in }
  }
  func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus) {
    self.saveStatus = saveStatus
    if case .failed(let failure) = saveStatus {
      message = (failure == .full ? session.storageNotice : nil) ?? failure.localizedDescription
    }
    if case .saved = saveStatus { widgets.saved(session) }
  }
  func pageSession(_ session: DocumentSession, storageFailure: TinyFailureContext) {
    message = "Document storage needs attention."
  }
  func pageSession(_ session: DocumentSession, themeChanged theme: TinyThemeState) {}
  func pageSession(_ session: DocumentSession, didReport issue: TinyPageIssue) { message = issue.message }
  func pageSession(_ session: DocumentSession, didFail error: Error) {
    if self.session === session { opening = false }
    message = error.localizedDescription
  }
  func pageSession(_ session: DocumentSession, didReplace webView: WKWebView) { objectWillChange.send() }
  func pageSession(_ session: DocumentSession, resizeContentTo size: CGSize) throws -> CGSize {
    session.webView.bounds.size
  }
}
