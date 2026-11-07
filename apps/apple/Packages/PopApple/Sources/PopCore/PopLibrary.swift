import Combine
import Foundation
import PopCoreBinding
import SwiftUI
import Synchronization

public func libraryMessage(_ error: Error) -> String {
  if case LibraryError.Failed(_, let message) = error { return message }
  return error.localizedDescription
}
private final class LibrarySnapshotListener: LibraryListener, @unchecked Sendable {
  private let invalidate: @Sendable () -> Void
  init(_ invalidate: @escaping @Sendable () -> Void) { self.invalidate = invalidate }
  func changed() { invalidate() }
}

extension LibraryEntry: Identifiable {}

/// Connects the library to document owners regardless of which finishes opening first.
final class LibraryOwners: Sendable {
  private struct OwnerReference { weak var owner: NativeOwner? }
  private struct State {
    var library: NativeLibrary?
    var owners: [OwnerReference] = []
  }
  private let active = Mutex(State())
  var library: NativeLibrary? { active.withLock { $0.library } }
  func register(_ owner: NativeOwner) throws {
    try active.withLock { state in
      state.owners.removeAll { $0.owner == nil }
      state.owners.append(OwnerReference(owner: owner))
      try state.library?.registerOwner(owner: owner)
    }
  }
  func activate(_ library: NativeLibrary?) throws {
    try active.withLock { state in
      state.owners.removeAll { $0.owner == nil }
      for reference in state.owners {
        if let owner = reference.owner { try library?.registerOwner(owner: owner) }
      }
      state.library = library
    }
  }
}

public struct LibrarySyncAttention {
  public let explanation: String
  public let report: String
}

/// Presentation of the Rust local index and cloud-document coordinator.
/// The Firebase adapter supplies identity and discovery; document bytes stay in Rust.
@MainActor public final class PopLibrary: ObservableObject {
  public static let shared = PopLibrary(owners: active)
  nonisolated private static let active = LibraryOwners()
  public nonisolated static func register(owner: NativeOwner) throws { try active.register(owner) }
  public nonisolated static func publishArtwork(owner: NativeOwner, sourceVersion: String, preview: Data?, icon: Data?)
    async throws -> Bool
  {
    guard let library = active.library else { return false }
    return try await Task.detached(priority: .utility) {
      try library.publishArtwork(owner: owner, sourceVersion: sourceVersion, preview: preview, icon: icon)
    }.value
  }
  @Published public private(set) var hasLoaded = false
  @Published public private(set) var accountIdentity = ""
  @Published public private(set) var entries: [LibraryEntry] = []
  @Published public private(set) var guestDocumentIDs: Set<String> = []
  @Published public private(set) var accountBoundPaths: Set<String> = []
  @Published public private(set) var issues: [String: String] = [:]
  @Published public private(set) var status = "Saved on this device"
  @Published public private(set) var isSyncing = false
  @Published public private(set) var isReady = false
  @Published public private(set) var lastSuccess: Date?
  @Published public private(set) var lastError: String?
  @Published public private(set) var pending = 0
  @Published public private(set) var foregroundAt = Date()
  public var editorOpen = false
  public var visibleEntries: [LibraryEntry] { entries.filter { !$0.deleted } }
  public var attention: LibrarySyncAttention? {
    lastError == nil && issues.isEmpty ? nil : LibrarySyncAttention(explanation: syncExplanation, report: syncReport)
  }
  public var syncExplanation: String { lastError ?? "Some changes could not sync. Your local files are retained." }
  public var syncReport: String {
    "pop sync\nCaptured: \(Date().ISO8601Format())\nPending: \(pending)\n\(syncExplanation)\n"
      + issues.sorted { $0.key < $1.key }.map { "\($0.key): \($0.value)" }.joined(separator: "\n")
  }
  public var closeAccountDocuments: (@MainActor ([String]) async throws -> Void)?
  public var concealAccountDocuments: (@MainActor ([String], Bool) -> Void)?
  public var catalogPublisher: (@MainActor (_ entry: DocumentEntry, _ account: String) async throws -> Void)?
  public var allowsCloudCreation = true
  public var tokenProvider: (@MainActor () async throws -> String)?
  private var native: NativeLibrary?
  private var pendingAccount: String?
  private var concealedPaths: [String] = []
  private var generation: UInt64 = 0
  private var task: Task<Void, Never>?
  private var syncTask: Task<Void, Never>?
  private var syncAgain = false
  private var snapshotChanges: AnyCancellable?
  private var snapshotListener: LibrarySnapshotListener?
  private var snapshotRefresh: Task<Void, Never>?
  private var refreshAgain = false
  private let root: URL
  private let owners: LibraryOwners
  init(root: URL = PopLibrary.managedFolder, owners: LibraryOwners = LibraryOwners()) {
    self.root = root
    self.owners = owners
  }
  public nonisolated static var managedFolder: URL {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("pop/CloudDocuments", isDirectory: true)
  }
  public func start() {
    guard task == nil else { return }
    snapshotChanges = NotificationCenter.default.publisher(for: .popDocumentSaveStatusChanged)
      .sink { @Sendable [weak self] _ in
        Task { @MainActor [weak self] in self?.requestRefresh() }
      }
    task = Task {
      if native == nil { try? await setAccount("") }
      while !Task.isCancelled {
        await refresh()
        try? await Task.sleep(for: .seconds(2))
      }
    }
  }
  /// A failed flush keeps the old account and its writers. Call before signing out.
  public func setAccount(_ account: String, external: Bool = false) async throws {
    if native != nil && accountIdentity == account {
      guard !isReady else { return }
      generation += 1
      let current = generation
      let path = root.path
      let restored = try await PopPreparation.run { try NativeLibrary.open(root: path, account: account) }
      guard current == generation else {
        try? restored.detach()
        return
      }
      native = restored
      observeSnapshots(restored, generation: current)
      try owners.activate(restored)
      pendingAccount = nil
      isReady = true
      concealAccountDocuments?(concealedPaths, false)
      concealedPaths = []
      lastError = nil
      await refresh()
      return
    }
    pendingAccount = external ? account : nil
    generation += 1
    let current = generation
    syncTask?.cancel()
    syncTask = nil
    isSyncing = false
    syncAgain = false
    let cloudPaths = Array(Set(entries.filter { $0.accountBound }.map(\.path) + concealedPaths))
    if external {
      isReady = false
      concealedPaths = cloudPaths
      concealAccountDocuments?(cloudPaths, true)
      entries.removeAll { $0.accountBound }
      // Auth is already lost: fence transport before attempting the normal page close.
      if let native { try await PopPreparation.run { try native.detach() } }
    }
    do { try await closeAccountDocuments?(cloudPaths) } catch {
      // Normal close restores a failed window for retry. Lost auth still conceals its page.
      if external { concealAccountDocuments?(cloudPaths, true) }
      throw error
    }
    guard current == generation else { throw PopFailure("The account changed. Retry.") }
    if !external, let native { try await PopPreparation.run { try native.detach() } }
    guard current == generation else { throw PopFailure("The account changed. Retry.") }
    try owners.activate(nil)
    let location = root.path
    let opened = try await PopPreparation.run { try NativeLibrary.open(root: location, account: account) }
    guard current == generation else {
      try? opened.detach()
      return
    }
    native = opened
    observeSnapshots(opened, generation: current)
    accountIdentity = account
    pendingAccount = nil
    concealedPaths = []
    lastSuccess = nil
    lastError = nil
    try owners.activate(opened)
    isReady = true
    await refresh()
  }
  @discardableResult
  public func receiveCatalog(_ catalog: [DocumentEntry], account: String) async throws -> UInt32 {
    guard account == accountIdentity, let native else { return 0 }
    let current = generation
    let outcome = try await PopPreparation.run { try native.receiveCatalog(entries: catalog) }
    guard current == generation else { return 0 }
    try owners.activate(native)
    await refresh()
    return outcome.rejected
  }
  public func reportCatalogError(_ error: Error, account: String) {
    guard account == accountIdentity || account == pendingAccount else { return }
    lastError = libraryMessage(error)
    status = "Sync needs attention"
  }
  private func observeSnapshots(_ library: NativeLibrary, generation current: UInt64) {
    let listener = LibrarySnapshotListener { [weak self] in
      Task { @MainActor [weak self] in
        guard let self, current == self.generation else { return }
        self.requestRefresh()
      }
    }
    snapshotListener = listener
    library.listen(listener: listener)
  }
  private func requestRefresh() {
    refreshAgain = true
    guard snapshotRefresh == nil else { return }
    snapshotRefresh = Task { [weak self] in
      guard let self else { return }
      while refreshAgain && !Task.isCancelled {
        refreshAgain = false
        await refresh()
      }
      snapshotRefresh = nil
    }
  }
  public func refresh() async {
    guard isReady, let native else { return }
    let current = generation
    do {
      let (loaded, accountPaths) = try await PopPreparation.run {
        try native.reconcile()
        return (try native.entries(), try native.accountPaths())
      }
      guard current == generation else { return }
      accountBoundPaths = Set(accountPaths.map { URL(fileURLWithPath: $0).standardizedFileURL.path })
      entries = loaded
      guestDocumentIDs = Set(loaded.filter { $0.role == .editor && !$0.deleted }.map(\.id))
      issues = Dictionary(uniqueKeysWithValues: loaded.compactMap { entry in entry.problem.map { (entry.id, $0) } })
      pending = loaded.filter { $0.dirty && !$0.localOnly }.count
      hasLoaded = true
      status =
        isSyncing
        ? "Syncing…"
        : lastError != nil
          ? "Sync needs attention"
          : pending > 0
            ? "Saved here · waiting to sync" : accountIdentity.isEmpty ? "Saved on this device" : "Sync across devices"
    } catch { lastError = libraryMessage(error) }
  }
  private func requiredToken() async throws -> String {
    guard isReady, !accountIdentity.isEmpty, let tokenProvider else {
      throw PopFailure("Sign in to sync across devices.")
    }
    return try await tokenProvider()
  }
  public func syncNow() async {
    if let pendingAccount {
      do { try await setAccount(pendingAccount, external: !isReady) } catch {
        lastError = libraryMessage(error)
        return
      }
    }
    if let syncTask {
      syncAgain = true
      await syncTask.value
      return
    }
    guard let native, !accountIdentity.isEmpty else {
      await refresh()
      return
    }
    let current = generation
    syncAgain = true
    let work = Task {
      isSyncing = true
      defer { if current == generation { isSyncing = false } }
      while syncAgain && current == generation && !Task.isCancelled {
        syncAgain = false
        var failure: String?
        do {
          let token = try await requiredToken()
          guard current == generation, !Task.isCancelled else { return }
          try await PopPreparation.run { try native.sync(token: token) }
        } catch { failure = libraryMessage(error) }
        guard current == generation, !Task.isCancelled else { return }
        do { try await publishCatalog(using: native, generation: current) } catch { failure = libraryMessage(error) }
        guard current == generation, !Task.isCancelled else { return }
        lastError = failure
        if failure == nil { lastSuccess = Date() }
      }
      await refresh()
    }
    syncTask = work
    await work.value
    if current == generation { syncTask = nil }
  }
  private func publishCatalog(using library: NativeLibrary, generation current: UInt64) async throws {
    guard let catalogPublisher else { return }
    let account = accountIdentity
    let writes = try await PopPreparation.run { try library.pendingCatalogWrites() }
    for entry in writes {
      guard current == generation, !Task.isCancelled else { return }
      try await catalogPublisher(entry, account)
      guard current == generation, account == accountIdentity, !Task.isCancelled else { return }
      try await PopPreparation.run {
        try library.ackCatalogWrite(documentId: entry.documentId, catalogRevision: entry.catalogRevision)
      }
    }
  }
  private func publishWhenPossible() { Task { await syncNow() } }

  public func fetchInBackground() async throws {
    await syncNow()
    if let lastError { throw PopFailure(lastError) }
  }
  public func saveToCloud() async { await syncNow() }
  public func activated(_ active: Bool) {
    if active {
      foregroundAt = Date()
      start()
      Task {
        await syncNow()
      }
    }
  }
  public func create(template: URL, title: String, sync: Bool = false) async throws -> LibraryEntry {
    if native == nil { try await setAccount("") }
    guard let native else { throw PopFailure("The local library is unavailable.") }
    let entry = try await PopPreparation.run { try native.create(template: template.path, title: title) }
    await refresh()
    if sync { return try await enableSync(id: entry.id) }
    return entry
  }
  @discardableResult public func enableSync(id: String) async throws -> LibraryEntry {
    guard allowsCloudCreation else { throw PopFailure("Sign in with Apple to create a cloud document.") }
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    let entry = try await PopPreparation.run { try native.enableSync(id: id, token: token) }
    publishWhenPossible()
    await refresh()
    return entry
  }
  public func enableSync(source: URL, title: String) async throws {
    guard allowsCloudCreation else { throw PopFailure("Sign in with Apple to create a cloud document.") }
    guard let native else { throw PopFailure("Sign in to sync across devices.") }
    let current = generation
    let entry = try await PopPreparation.run { try native.adopt(source: source.path, title: title) }
    guard current == generation else { throw PopFailure("The account changed. Retry.") }
    try owners.activate(native)
    _ = try await enableSync(id: entry.id)
  }
  public func delete(_ entry: LibraryEntry) async throws {
    guard let native else { return }
    if entry.localOnly {
      try await PopPreparation.run { try native.deleteLocal(id: entry.id) }
    } else {
      let token = try await requiredToken()
      if entry.role == .editor {
        try await PopPreparation.run { try native.leave(id: entry.id, token: token) }
      } else {
        try await PopPreparation.run { try native.deleteEverywhere(id: entry.id, token: token) }
      }
    }
    publishWhenPossible()
    await refresh()
  }
  public func removeDownload(_ entry: LibraryEntry) async throws {
    guard let native else { return }
    try await PopPreparation.run { try native.removeDownload(id: entry.id) }
    await refresh()
  }
  public func download(_ entry: LibraryEntry) async throws {
    let token = try await requiredToken()
    guard let native else { return }
    _ = try await PopPreparation.run { try native.download(id: entry.id, token: token) }
    await refresh()
  }
  @discardableResult public func localCopy(_ entry: LibraryEntry) async throws -> LibraryEntry {
    guard let native else { throw PopFailure("The library is unavailable.") }
    let copy = try await PopPreparation.run { try native.localCopy(id: entry.id, title: entry.title + " copy") }
    await refresh()
    return copy
  }
  public func sharing(id: String, enabled: Bool) async throws -> AccessState {
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    let access = try await PopPreparation.run { try native.sharing(id: id, enabled: enabled, token: token) }
    publishWhenPossible()
    return access
  }
  public func join(link: String) async throws -> LibraryEntry {
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    let entry = try await PopPreparation.run { try native.join(link: link, token: token) }
    publishWhenPossible()
    await refresh()
    return entry
  }
  public func access(id: String) async throws -> AccessState {
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    return try await PopPreparation.run { try native.access(id: id, token: token) }
  }
  public func pauseDocument(id: String) async throws {
    guard let native else { return }
    try await PopPreparation.run { try native.unregisterOwner(id: id) }
  }
  public func connection(id: String) -> NativeSharing? { native?.connection(id: id) }
  public func entry(at url: URL) -> LibraryEntry? {
    entries.first { URL(fileURLWithPath: $0.path).standardizedFileURL == url.standardizedFileURL }
  }
  public func isSynced(_ entry: LibraryEntry) -> Bool {
    !entry.localOnly && entry.downloaded && !entry.dirty && entry.problem == nil
  }
  public func state(for entry: LibraryEntry) -> String {
    if entry.localOnly { return "Saved on this device" }
    if entry.problem != nil { return "Sync needs attention" }
    if !entry.downloaded { return "Tap to download" }
    return isSynced(entry) ? "Synced across devices" : "Saved here · waiting to sync"
  }
}
