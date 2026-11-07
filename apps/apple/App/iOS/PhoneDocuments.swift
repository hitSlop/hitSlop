import PopCore
import PopCoreBinding
import PopDocument
import SwiftUI
import UIKit
import WebKit

struct PhoneRecent: Identifiable, Sendable {
  let url: URL
  let title: String
  let entry: LibraryEntry?
  var documentID: String? = nil
  var isUnavailable = false
  /// Why this build will not open the file, such as one an older version made; it is never modified.
  var refusal: String? = nil
  var id: String { Self.key(url) }
  /// One document's identity in the catalog, its transition and its close.
  static func key(_ url: URL) -> String { url.standardizedFileURL.path }

  static func isSharedDocument(_ url: URL) -> Bool {
    url.deletingLastPathComponent().lastPathComponent == "SharedDocuments"
      && UUID(uuidString: url.deletingPathExtension().lastPathComponent) != nil
  }

  /// Shared copies keep their UUID path for canonical joins; their app supplies the visible name.
  static func localTitle(_ url: URL) -> String {
    if url.deletingPathExtension().lastPathComponent.contains("-recovery-"),
      let title = try? PopSummary(url: url).metadata.title
    {
      return "Recovered — \(title)"
    }
    if isSharedDocument(url) {
      guard let title = try? PopSummary(url: url).metadata.title,
        !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      else { return "Shared pop" }
      return title
    }
    var name = url.deletingPathExtension().lastPathComponent
    if let dash = name.lastIndex(of: "-") {
      let suffix = name[name.index(after: dash)...]
      if suffix.count == 8 && suffix.allSatisfy({ $0.isHexDigit }) { name = String(name[..<dash]) }
    }
    return name
  }
}

/// A document that left the screen saves and releases in the background. Its tile shows
/// this until the close ends; a close that fails keeps the document's owner for Retry.
enum PhoneClosing: Equatable {
  case saving
  case failed(String)
}

@MainActor final class PhoneDocuments: ObservableObject, DocumentSessionDelegate {
  @Published var accountRequired = false
  @Published var documents: [URL] = []
  @Published var templates: [PhoneTemplate] = []
  @Published var unavailableDocuments: Set<URL> = []
  @Published private var refusedDocuments: [URL: String] = [:]
  @Published var localDocumentIDs: [URL: String] = [:]
  @Published var localTitles: [URL: String] = [:]
  @Published private var fileDates: [URL: Date] = [:]
  @Published private var openedAt: [String: Double] = [:]
  /// The document on screen, from the tap that opened it until it leaves.
  @Published private(set) var presented: URL?
  /// The document on screen's owner; nil while it downloads or prepares.
  @Published var session: DocumentSession?
  /// The document on screen shows its artwork until its page is ready and padded.
  @Published var opening = false
  /// A document off screen until its close ends. A failed close keeps its owner, with every
  /// edit and the save status it last reported, for Retry or for coming back on screen.
  private struct Departing {
    enum Phase {
      case closing(Task<Bool, Never>)
      case failed(String)
      /// Released; the library refresh still runs, and reopening or library actions wait for it.
      case finishing(Task<Bool, Never>)
    }
    let session: DocumentSession
    var phase: Phase
    var status: DocumentSaveStatus = .saved
    /// False when the person left before its page loaded: coming back loads it, not reopens it.
    var loaded = true
    /// The close in progress, through its library refresh.
    var task: Task<Bool, Never>? {
      switch phase {
      case .closing(let task), .finishing(let task): task
      case .failed: nil
      }
    }
  }
  @Published private var departing: [String: Departing] = [:]
  /// Documents that left the screen and are still saving, or failed to.
  var closing: [String: PhoneClosing] {
    departing.compactMapValues {
      switch $0.phase {
      case .closing: .saving
      case .failed(let message): .failed(message)
      case .finishing: nil
      }
    }
  }
  /// The document whose page did not open or stopped; the alert offers Reopen.
  @Published private(set) var pageFailure: URL?
  /// Recents keep their order while a document is on screen, so its zoom returns to the
  /// tile it came from; the document moves to the front once it has landed.
  @Published private var heldOrder: [String: Int]?
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
  /// A light tap for a deliberate host action: opening a pop, a menu choice.
  @Published var tapFeedback = 0
  /// Soft taps as a document leaves and as its page appears; an error tap when a close fails.
  @Published private(set) var leaveFeedback = 0
  @Published private(set) var readyFeedback = 0
  @Published private(set) var closeFailureFeedback = 0
  func tapped() { tapFeedback += 1 }
  private var manualSync = false
  @Published var message: String?
  @Published var showImporter = false
  @Published var showTheme = false
  @Published var exported: URL?
  private var pendingLibraryID: String?
  /// Opens in flight by document; a document reopened at once waits for the open it left.
  private var opens: [String: (id: UUID, task: Task<Void, Never>)] = [:]
  /// Opens a document's owner. Tests hold an open in flight here.
  var openSession: @MainActor (URL) async throws -> DocumentSession = { try await DocumentSession.open(url: $0) }
  private(set) lazy var widgets = PhoneWidgetPublisher { [weak self] in self?.session(showing: $0) }
  private var backgroundID: UIBackgroundTaskIdentifier = .invalid
  private var backgroundSave: Task<Void, Never>?
  private var refreshTask: Task<Void, Never>?
  private static let recentsKey = "click.dotpop.phone.recents.openedAt"
  private let folder = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("Documents", isDirectory: true)

  init() {
    openedAt = UserDefaults.standard.dictionary(forKey: Self.recentsKey) as? [String: Double] ?? [:]
    _ = widgets
    PopRegistry.sweep()
    do {
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      refresh()
    } catch { message = error.localizedDescription }
  }
  func loadTemplates() async {
    let root = Bundle.main.resourceURL?.appendingPathComponent("StarterTemplates", isDirectory: true)
    guard let root else { return }
    do {
      templates = try await PopPreparation.run {
        try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
          .filter { $0.pathExtension == "pop" }.map { url in
            let summary = try PopSummary(url: url, template: true)
            return PhoneTemplate(
              url: url, title: summary.metadata.title, description: summary.metadata.description,
              categories: summary.metadata.categories.map { $0.rawValue.capitalized })
          }
      }
    } catch { message = error.localizedDescription }
  }
  func create(_ template: PhoneTemplate, name: String, sync: Bool = false) {
    perform {
      let entry = try await PopLibrary.shared.create(template: template.url, title: name, sync: sync)
      self.present(URL(fileURLWithPath: entry.path))
      self.successFeedback += 1
    }
  }
  func recents(in entries: [LibraryEntry]) -> [PhoneRecent] {
    let synced = entries.map { entry in
      let url = URL(fileURLWithPath: entry.path)
      return PhoneRecent(
        url: url, title: entry.title, entry: entry,
        isUnavailable: entry.downloaded && unavailableDocuments.contains(url), refusal: refusedDocuments[url])
    }
    let managed = Set(synced.map(\.id))
    let local = documents.compactMap { url -> PhoneRecent? in
      guard !managed.contains(url.standardizedFileURL.path) else { return nil }
      return PhoneRecent(
        url: url,
        title: localTitles[url]
          ?? (PhoneRecent.isSharedDocument(url) ? "Shared pop" : url.deletingPathExtension().lastPathComponent),
        entry: nil, documentID: localDocumentIDs[url], isUnavailable: unavailableDocuments.contains(url),
        refusal: refusedDocuments[url])
    }
    func activity(_ recent: PhoneRecent) -> Date {
      max(
        fileDates[recent.url] ?? .distantPast,
        openedAt[recent.id].map(Date.init(timeIntervalSince1970:)) ?? .distantPast)
    }
    let sorted = (synced + local).sorted {
      let first = activity($0)
      let second = activity($1)
      if first != second { return first > second }
      let titleOrder = $0.title.localizedStandardCompare($1.title)
      return titleOrder == .orderedSame ? $0.id < $1.id : titleOrder == .orderedAscending
    }
    guard let heldOrder else { return sorted }
    // Documents that appeared meanwhile (a new pop) lead, in their own order.
    return sorted.enumerated().sorted {
      (heldOrder[$0.element.id] ?? -1, $0.offset) < (heldOrder[$1.element.id] ?? -1, $1.offset)
    }.map(\.element)
  }
  /// The document screen has left after its transition: Recents take their current order.
  func documentScreenLeft() {
    guard presented == nil, heldOrder != nil else { return }
    withAnimation(.snappy) { heldOrder = nil }
  }
  func delete(_ recent: PhoneRecent) {
    perform {
      try await self.settled(recent.url)
      if let session = self.session, session.file.url == recent.url {
        let drafts = try await session.prepareClose()
        guard drafts.isEmpty else {
          await session.cancelClose()
          throw PopFailure("Save your pending text before deleting.")
        }
        try await session.close()
        self.session = nil
        self.presented = nil
        self.opening = false
      }
      if let entry = recent.entry {
        try await PopLibrary.shared.delete(entry)
      } else {
        try await PopPreparation.run { try PopFile.delete(recent.url) }
      }
      self.openedAt.removeValue(forKey: recent.id)
      UserDefaults.standard.set(self.openedAt, forKey: Self.recentsKey)
      self.refresh()
      self.deletionFeedback += 1
    }
  }
  func saveLocalCopy(_ entry: LibraryEntry) {
    perform {
      try await self.settled(URL(fileURLWithPath: entry.path))
      let copy = try await PopLibrary.shared.localCopy(entry)
      self.present(URL(fileURLWithPath: copy.path))
    }
  }
  func enableSync(_ entry: LibraryEntry) {
    perform {
      try await self.settled(URL(fileURLWithPath: entry.path))
      _ = try await PopLibrary.shared.enableSync(id: entry.id)
    }
  }
  func removeDownload(_ entry: LibraryEntry) {
    perform {
      try await self.settled(URL(fileURLWithPath: entry.path))
      try await PopLibrary.shared.removeDownload(entry)
      self.refresh()
      self.deletionFeedback += 1
    }
  }
  /// Only explicit sync requests produce feedback; polling and background sync do not.
  func syncNow() async {
    guard !manualSync else { return }
    manualSync = true
    defer { manualSync = false }
    let library = PopLibrary.shared
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
    if PopLibrary.shared.hasLoaded, let id = pendingLibraryID {
      pendingLibraryID = nil
      open(libraryID: id)
    }
    guard let session, PopLibrary.shared.entry(at: session.file.url)?.deleted == true, !busy else { return }
    perform {
      let drafts = try await session.prepareClose()
      guard drafts.isEmpty else {
        await session.cancelClose()
        throw PopFailure("This pop was deleted on another device. Keep it open until your text is saved.")
      }
      try await session.close()
      if self.session === session {
        self.session = nil
        self.presented = nil
        self.opening = false
      }
      self.message = "Deleted on another device. Any unsynced edits are kept as a local pop."
    }
  }
  func saveOnInactive() {
    guard let session, backgroundSave == nil else { return }
    Task { do { try await session.saveAccepted() } catch { message = error.localizedDescription } }
  }
  private func refresh() {
    let shared = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("pop/SharedDocuments", isDirectory: true)
    let local = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    documents = Array(
      Set(
        [folder, shared, local].flatMap {
          (try? FileManager.default.contentsOfDirectory(at: $0, includingPropertiesForKeys: nil)) ?? []
        })
    ).filter { $0.pathExtension == "pop" && !PopLibrary.shared.accountBoundPaths.contains($0.standardizedFileURL.path) }
      .sorted { $0.lastPathComponent < $1.lastPathComponent }
    let paths = Set(documents + PopLibrary.shared.visibleEntries.map { URL(fileURLWithPath: $0.path) })
    refreshTask?.cancel()
    let previousDocumentIDs = localDocumentIDs
    refreshTask = Task {
      let metadata = await Task.detached(priority: .utility) {
        var titles: [URL: String] = [:]
        var dates: [URL: Date] = [:]
        var documentIDs: [URL: String] = [:]
        var unavailable: Set<URL> = []
        var refused: [URL: String] = [:]
        for url in paths {
          titles[url] = PhoneRecent.localTitle(url)
          if let id = try? sharingLocalDocument(path: url.path) {
            documentIDs[url] = id
          } else {
            unavailable.insert(url)
            documentIDs[url] = previousDocumentIDs[url]
            // A file left by an older version, for example, says why instead of offering Reopen.
            do { _ = try PopSummary(url: url) } catch { refused[url] = PopFile.refusal(error) }
          }
          let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .creationDateKey])
          dates[url] = max(values?.contentModificationDate ?? .distantPast, values?.creationDate ?? .distantPast)
        }
        return (titles, dates, documentIDs, unavailable, refused)
      }.value
      guard !Task.isCancelled else { return }
      localTitles = metadata.0
      fileDates = metadata.1
      localDocumentIDs = metadata.2
      unavailableDocuments = metadata.3
      refusedDocuments = metadata.4
    }
  }
  private func perform(_ operation: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true
    Task {
      do { try await operation() } catch { message = libraryMessage(error) }
      busy = false
    }
  }
  func importDocument(_ source: URL) {
    guard session == nil else {
      message = "Close your current document before importing another."
      return
    }
    perform {
      let target = self.folder.appendingPathComponent(
        source.deletingPathExtension().lastPathComponent + "-" + UUID().uuidString.prefix(8) + ".pop")
      let imported = try await PopPreparation.run { () throws -> URL in
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        var outcome: Result<URL, Error>?
        var coordinationError: NSError?
        NSFileCoordinator().coordinate(readingItemAt: source, options: [], error: &coordinationError) { resolved in
          outcome = Result { try PopFile.importCopy(from: resolved, to: target) }
        }
        if let coordinationError { throw coordinationError }
        guard let outcome else { throw PopFailure("Files did not provide this document. Try downloading it first.") }
        return try outcome.get()
      }
      self.refresh()
      self.present(imported)
    }
  }
  /// A tile was tapped: the document is on screen at once, zooming out of the tile.
  func open(_ url: URL) {
    tapped()
    present(url)
  }
  /// A tile was tapped. A file this build refuses says why and stays where it is.
  func open(_ recent: PhoneRecent) {
    guard let refusal = recent.refusal else { return open(recent.url) }
    tapped()
    message = refusal
  }
  func open(libraryID: String) {
    guard PopLibrary.shared.hasLoaded else {
      pendingLibraryID = libraryID
      PopLibrary.shared.start()
      return
    }
    // Resolve before leaving, so a stale widget never costs the open document.
    guard let entry = PopLibrary.shared.visibleEntries.first(where: { $0.id == libraryID }) else {
      message = "This document is no longer in your library."
      return
    }
    present(URL(fileURLWithPath: entry.path))
  }
  func join(_ link: String) async throws {
    if let session {
      let incoming = try await PopPreparation.run { try sharingDocument(link: link) }
      guard incoming == session.documentID else {
        throw PopFailure("Close your current document before joining another.")
      }
    }
    let url = try await DocumentSharing.join(link)
    refresh()
    if session?.file.url != url { present(url) }
    successFeedback += 1
  }

  /// The live session for the document at `url`: the one on screen, or one still closing.
  func session(showing url: URL) -> DocumentSession? {
    let key = PhoneRecent.key(url)
    if let session, PhoneRecent.key(session.file.url) == key { return session }
    guard let record = departing[key] else { return nil }
    if case .finishing = record.phase { return nil }
    return record.session
  }
  /// Whether `session`'s page is ready to be seen: a page still opening stays behind the
  /// document's artwork until it is ready and has its safe-area padding.
  func shows(_ session: DocumentSession) -> Bool {
    session.isReady && !(session === self.session && opening)
  }

  /// Shows `url` from anywhere (a tile, a widget, a link, a new or imported pop) at once,
  /// over its artwork, and opens it behind. Another document on screen leaves first, the
  /// way swiping back leaves it. A document still closing opens when its close ends; one
  /// whose close failed comes back with its live page and every edit.
  func present(_ url: URL) {
    let key = PhoneRecent.key(url)
    if let presented, PhoneRecent.key(presented) == key { return }
    if presented != nil || session != nil { leave() } else { holdOrder() }
    pageFailure = nil
    presented = url
    opening = true
    let previous = opens[key]?.task
    let id = UUID()
    let task = Task { [weak self] in
      await previous?.value
      await self?.load(url, key: key)
      if self?.opens[key]?.id == id { self?.opens[key] = nil }
    }
    opens[key] = (id, task)
  }
  private func holdOrder() {
    heldOrder = nil
    heldOrder = Dictionary(
      recents(in: PopLibrary.shared.visibleEntries).enumerated().map { ($1.id, $0) }, uniquingKeysWith: min)
  }
  private func load(_ url: URL, key: String) async {
    while !Task.isCancelled, let close = departing[key]?.task { _ = await close.value }
    guard !Task.isCancelled, presented.map(PhoneRecent.key) == key else { return }
    // What is left of a close here failed; its owner comes back with every edit.
    if let retained = departing.removeValue(forKey: key) {
      saveStatus = retained.status
      session = retained.session
      if retained.session.isReady {
        reveal(retained.session)
      } else if !retained.loaded {
        retained.session.load()
      } else {
        failPage(url, message: "This pop's page stopped. Reopen it to keep editing.")
      }
      return
    }
    do {
      try await show(url, key: key)
    } catch is CancellationError {
    } catch {
      guard presented.map(PhoneRecent.key) == key else { return }
      // Reopening cannot help a file this build refuses.
      let refusal = PopFile.refusal(error)
      failPage(url, message: refusal ?? libraryMessage(error), reopenable: refusal == nil)
    }
  }
  private func show(_ url: URL, key: String) async throws {
    if let entry = PopLibrary.shared.entry(at: url), !entry.downloaded {
      try await PopLibrary.shared.download(entry)
    }
    try Task.checkCancellation()
    let opened = try await openSession(url)
    opened.delegate = self
    guard !Task.isCancelled, presented.map(PhoneRecent.key) == key, session == nil else {
      // The person left while it opened. Release it the usual way, unseen.
      close(opened, key: key, loaded: false)
      throw CancellationError()
    }
    session = opened
    openedAt[key] = Date().timeIntervalSince1970
    UserDefaults.standard.set(openedAt, forKey: Self.recentsKey)
    saveStatus = .saved
    opened.load()
  }
  /// Back to the library with the alert; a session whose page failed keeps its owner, so
  /// Reopen attaches a new page to it.
  private func failPage(_ url: URL, message text: String, reopenable: Bool = true) {
    pageFailure = reopenable ? url : nil
    if opening {
      presented = nil
      opening = false
    }
    message = text
  }
  /// The alert's Reopen: a new page for the kept owner, or the whole open again.
  func reopenFailedPage() {
    guard let url = pageFailure else { return }
    pageFailure = nil
    guard let session, PhoneRecent.key(session.file.url) == PhoneRecent.key(url) else { return present(url) }
    if presented == nil { holdOrder() }
    presented = url
    guard !session.isReady else { return }
    opening = true
    Task {
      do { try await session.reopenSavedDocument() } catch {
        // A page that fails again reports through `pageSession(_:didFail:)`.
        NSLog("pop reopen failed: %@", error.localizedDescription)
      }
    }
  }
  /// The alert's Close: the document with the failed page leaves, through the usual close.
  func dismissPageFailure() {
    pageFailure = nil
    leave()
  }

  /// The one way out of a document: swiping back, Back to Library, another document taking
  /// its place, signing out. The catalog is usable at once; the close runs on its own and
  /// is returned. Leaving again, or with nothing on screen, starts nothing new.
  @discardableResult func leave() -> Task<Bool, Never>? {
    let key = presented.map(PhoneRecent.key)
    if let key { opens[key]?.task.cancel() }
    presented = nil
    opening = false
    guard let session else { return nil }
    self.session = nil
    leaveFeedback += 1
    return close(session, key: key ?? PhoneRecent.key(session.file.url))
  }
  /// Retry on a tile whose close failed: the same close, with the owner it kept.
  @discardableResult func retryClose(_ url: URL) -> Task<Bool, Never>? {
    let key = PhoneRecent.key(url)
    guard let record = departing[key], case .failed = record.phase else { return nil }
    tapped()
    return close(record.session, key: key)
  }
  /// Signing out closes each of the account's documents: on screen, still opening or still
  /// closing. Opens in flight end first, so no session of the account outlives the sign-out.
  func leave(paths: Set<String>) async -> Bool {
    let keys = Set(paths.map { PhoneRecent.key(URL(fileURLWithPath: $0)) })
    let onScreen = [presented, session?.file.url].compactMap { $0 }.map(PhoneRecent.key)
    if onScreen.contains(where: keys.contains) { leave() }
    // A cancelled open releases what it opened as a departing document, closed below.
    for (key, open) in opens where keys.contains(key) { await open.task.value }
    let closes = departing.filter { keys.contains($0.key) }.map { key, record in
      record.task ?? close(record.session, key: key)
    }
    var closed = true
    for close in closes {
      if !(await close.value) { closed = false }
    }
    return closed
  }
  /// Library actions on a tile wait for its document to finish closing.
  private func settled(_ url: URL) async throws {
    let key = PhoneRecent.key(url)
    while let close = departing[key]?.task { _ = await close.value }
    if departing[key] != nil { throw PopFailure("This pop's changes are not saved yet. Tap Retry on it first.") }
  }

  /// The page drains and saves first, at once, while it is still on screen animating out.
  /// Widget capture, artwork and release follow off the interactive path. A failure keeps
  /// the owner with every edit and marks the tile for Retry; nothing is discarded.
  @discardableResult private func close(
    _ session: DocumentSession, key: String, loaded: Bool = true
  ) -> Task<Bool, Never> {
    if let task = departing[key]?.task { return task }
    // The keyboard leaves with the page instead of returning over the library once the
    // transition ends: the field gives up focus (committing its text) before the drain.
    if let webView = session.currentWebView {
      webView.evaluateJavaScript("document.activeElement?.blur()", in: nil, in: .page) { _ in }
      webView.endEditing(true)
    }
    let trace = PopCloseTrace()
    let background = UIApplication.shared.beginBackgroundTask(withName: "Close document")
    let task = Task { [weak self] () -> Bool in
      defer {
        trace.finish()
        if background != .invalid { UIApplication.shared.endBackgroundTask(background) }
      }
      guard let self else { return false }
      do {
        try await finishLeaving(session, trace: trace)
      } catch {
        NSLog("pop close failed: %@", error.localizedDescription)
        if departing[key]?.session === session { departing[key]?.phase = .failed(libraryMessage(error)) }
        closeFailureFeedback += 1
        return false
      }
      if let task = departing[key]?.task, departing[key]?.session === session {
        departing[key]?.phase = .finishing(task)
      }
      refresh()
      await PopLibrary.shared.refresh()
      Task { await PopLibrary.shared.saveToCloud() }
      if departing[key]?.session === session { departing[key] = nil }
      return true
    }
    // Retry keeps the failed close's status and whether its page ever loaded.
    let failed = departing[key].flatMap { $0.session === session ? $0 : nil }
    departing[key] = Departing(
      session: session, phase: .closing(task), status: failed?.status ?? .saved, loaded: failed?.loaded ?? loaded)
    return task
  }
  private func finishLeaving(_ session: DocumentSession, trace: PopCloseTrace) async throws {
    let barrier = trace.begin("Save barrier")
    let drafts: [UnresolvedTextDraft]
    do { drafts = try await session.prepareClose() } catch {
      trace.end("Save barrier", barrier)
      throw error
    }
    trace.end("Save barrier", barrier)
    guard drafts.isEmpty else {
      await session.cancelClose()
      throw PopFailure("Some text could not be saved. Open this pop and copy that text before closing it.")
    }
    let capture = trace.begin("Widget capture")
    await widgets.closing(session)
    trace.end("Widget capture", capture)
    let render = trace.begin("Artwork render")
    do { _ = try await DocumentArtworkRenderer.refresh(session: session) } catch {
      NSLog("pop artwork refresh failed: %@", error.localizedDescription)
    }
    trace.end("Artwork render", render)
    try await session.close(trace: trace)
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
      let output = staging.appendingPathComponent(Self.backupName(session.file.url))
      do { self.exported = try await session.exportFile(to: output) } catch {
        try? FileManager.default.removeItem(at: staging)
        throw error
      }
    }
  }
  /// A backup in Files carries the name its tile shows; a library pop's file is named by its ID.
  static func backupName(_ url: URL) -> String {
    let title = PopLibrary.shared.entry(at: url)?.title ?? PhoneRecent.localTitle(url)
    let name = title.replacingOccurrences(of: "/", with: "-").replacingOccurrences(of: ":", with: "-")
      .trimmingCharacters(in: CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: ".")))
    return (name.isEmpty ? "pop" : String(name.prefix(120))) + ".pop"
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
      let target = directory.appendingPathComponent("\(name) copy \(UUID().uuidString.prefix(8)).pop")
      let copied = try await session.handOff(to: target)
      if self.session === session { self.session = nil }
      self.refresh()
      self.successFeedback += 1
      self.present(copied)
      await PopLibrary.shared.refresh()
      Task { await PopLibrary.shared.saveToCloud() }
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
        await PopLibrary.shared.saveToCloud()
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
    guard self.session === session else { return }
    reveal(session)
  }
  func pageSessionRecovered(_ session: DocumentSession) {
    guard self.session === session else { return }
    reveal(session)
  }
  /// The artwork gives way to the page only once the page is ready, padded for the safe
  /// area and has drawn that padding, so nothing shifts or flashes as it appears.
  private func reveal(_ session: DocumentSession) {
    publishSafeArea(session) { [weak self] in
      guard let self, self.session === session, opening, session.isReady else { return }
      opening = false
      readyFeedback += 1
    }
  }
  /// Publishes again when the window's insets change (the page keeps its own padding).
  func republishSafeArea(_ session: DocumentSession) {
    guard self.session === session, session.isReady else { return }
    publishSafeArea(session)
  }
  /// The edge-to-edge page pads itself by the status bar and home indicator heights. A ready
  /// page has its shell entry point; the key window knows the insets even before the page's
  /// view is attached, and WebKit reports none for a page that ignores the safe area.
  /// `drawn` runs after the page has drawn two frames with them, or after a quarter second.
  private func publishSafeArea(_ session: DocumentSession, drawn: @escaping @MainActor () -> Void = {}) {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    guard
      let window = (scenes.first { $0.activationState == .foregroundActive } ?? scenes.first)?.keyWindow,
      let webView = session.currentWebView
    else { return drawn() }
    let (top, bottom) = (Int(window.safeAreaInsets.top.rounded()), Int(window.safeAreaInsets.bottom.rounded()))
    Task {
      _ = try? await webView.callAsyncJavaScript(
        """
        globalThis.__pop?.safeArea?.(top, bottom);
        await new Promise(done => { requestAnimationFrame(() => requestAnimationFrame(done)); setTimeout(done, 250) });
        """, arguments: ["top": top, "bottom": bottom], in: nil, contentWorld: .page)
      drawn()
    }
  }
  /// Only the document on screen reports to the person; one closing reports through its tile
  /// and keeps its status for coming back. Any other session's status has nowhere to go.
  func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus) {
    guard self.session === session else {
      let key = PhoneRecent.key(session.file.url)
      if departing[key]?.session === session { departing[key]?.status = saveStatus }
      return
    }
    self.saveStatus = saveStatus
    if case .failed(let failure) = saveStatus {
      message = (failure == .full ? session.storageNotice : nil) ?? failure.localizedDescription
    }
    if case .saved = saveStatus { widgets.saved(session) }
  }
  func pageSession(_ session: DocumentSession, storageFailure: PopFailureContext) {
    guard self.session === session else { return NSLog("pop: a closing document's storage needs attention") }
    message = "Document storage needs attention."
  }
  func pageSession(_ session: DocumentSession, themeChanged theme: PopThemeState) {}
  func pageSession(_ session: DocumentSession, didReport issue: PopPageIssue) {
    guard self.session === session else { return NSLog("pop: closing document page: %@", issue.message) }
    message = issue.message
  }
  /// A page that fails while opening returns to the library with Reopen; one that stops
  /// while editing offers Reopen in place. A closing page's failure does not stop its close.
  func pageSession(_ session: DocumentSession, didFail error: Error) {
    guard self.session === session else {
      return NSLog("pop: closing document page failed: %@", error.localizedDescription)
    }
    failPage(session.file.url, message: error.localizedDescription)
  }
  func pageSession(_ session: DocumentSession, didReplace webView: WKWebView) { objectWillChange.send() }
  func pageSession(_ session: DocumentSession, resizeContentTo size: CGSize) throws -> CGSize {
    session.webView.bounds.size
  }
}
