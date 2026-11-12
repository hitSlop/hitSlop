import Combine
import Foundation
import PopCoreBinding
import SwiftUI

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

public struct LibrarySyncAttention {
  public let explanation: String
  public let report: String
}

/// Presentation of the Rust local index and cloud-document coordinator.
/// The Firebase adapter supplies identity and discovery; document bytes stay in Rust.
@MainActor public final class PopLibrary: ObservableObject {
  nonisolated private static let active: Result<NativeLibrary, Error> = Result {
    try NativeLibrary.open(root: managedFolder.path, account: "")
  }
  public static let shared = PopLibrary(native: active)
  public nonisolated static func register(owner: NativeOwner) throws {
    guard case .success(let library) = active else { return }
    try library.registerOwner(owner: owner)
  }
  public nonisolated static func publishArtwork(owner: NativeOwner, sourceVersion: String, preview: Data?, icon: Data?)
    async throws -> Bool
  {
    guard case .success(let library) = active else { return false }
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
  public var tokenProvider: (@MainActor () async throws -> String)?
  // Suspended work must never publish a snapshot from a departing account.
  private var accountEpoch: UInt64 = 0
  private var accountActive = true
  private var native: NativeLibrary?
  private var pendingAccount: String?
  private var concealedPaths: [String] = []
  private var task: Task<Void, Never>?
  private var syncTask: Task<Void, Never>?
  private var syncAgain = false
  private var snapshotChanges: AnyCancellable?
  private var snapshotListener: LibrarySnapshotListener?
  private var snapshotRefresh: Task<Void, Never>?
  private var refreshAgain = false
  private let root: URL
  private var watcher: LibraryFolderWatcher?
  init(root: URL = PopLibrary.managedFolder) {
    self.root = root
    let opened = Result { try NativeLibrary.open(root: root.path, account: "") }
    native = try? opened.get()
    isReady = native != nil
    if let native { observeSnapshots(native) }
    if case .failure(let error) = opened { lastError = libraryMessage(error) }
  }
  private init(native: Result<NativeLibrary, Error>) {
    root = Self.managedFolder
    self.native = try? native.get()
    isReady = self.native != nil
    if let native = self.native { observeSnapshots(native) }
    if case .failure(let error) = native { lastError = libraryMessage(error) }
  }
  func registerOwner(_ owner: NativeOwner) throws { try native?.registerOwner(owner: owner) }
  var nativeLibrary: NativeLibrary? { native }
  public nonisolated static var managedFolder: URL {
    if let registry = ProcessInfo.processInfo.environment["POP_TEST_REGISTRY"] {
      return URL(fileURLWithPath: registry).deletingLastPathComponent().appendingPathComponent(
        "library", isDirectory: true)
    }
    return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("pop/Library", isDirectory: true)
  }
  public func start() {
    guard task == nil else { return }
    snapshotChanges = NotificationCenter.default.publisher(for: .popDocumentSaveStatusChanged)
      .sink { @Sendable [weak self] _ in
        Task { @MainActor [weak self] in self?.requestRefresh() }
      }
    watcher = LibraryFolderWatcher(root: root) { [weak self] in self?.requestRefresh() }
    task = Task { await refresh() }
  }
  /// A failed close retains its owner and lets the person retry.
  public func setAccount(_ account: String, external: Bool = false) async throws {
    guard let native else { throw PopFailure("The local library is unavailable.") }
    if isReady && accountIdentity == account { return }
    accountEpoch &+= 1
    let epoch = accountEpoch
    let wasReady = isReady
    isReady = false
    pendingAccount = external ? account : nil
    syncTask?.cancel()
    syncTask = nil
    isSyncing = false
    syncAgain = false
    let cloudPaths = Array(Set(entries.filter { $0.accountBound }.map(\.path) + concealedPaths))
    if external {
      concealedPaths = cloudPaths
      concealAccountDocuments?(cloudPaths, true)
      entries.removeAll { $0.accountBound }
      if accountIdentity != account {
        try native.suspendAccount()
      }
    }
    if accountIdentity != account {
      do { try await closeAccountDocuments?(cloudPaths) } catch {
        guard epoch == accountEpoch else { throw CancellationError() }
        if external { concealAccountDocuments?(cloudPaths, true) } else { isReady = wasReady }
        throw error
      }
    }
    guard epoch == accountEpoch else { throw CancellationError() }
    do {
      try await PopPreparation.run(on: Self.queue) { try native.setAccount(account: account.isEmpty ? nil : account) }
    } catch {
      guard epoch == accountEpoch else { throw CancellationError() }
      if !external { isReady = wasReady }
      throw error
    }
    guard epoch == accountEpoch else { throw CancellationError() }
    native.setAccountActive(active: accountActive)
    accountIdentity = account
    pendingAccount = nil
    isReady = true
    concealAccountDocuments?(concealedPaths, false)
    concealedPaths = []
    lastError = nil
    await refresh()
    guard epoch == accountEpoch else { throw CancellationError() }
  }
  private nonisolated static let queue = DispatchQueue(label: "click.dotpop.library", qos: .utility)
  public func connectAccount(_ account: String) async throws {
    try await setAccount(account, external: true)
  }
  public func reportAccountError(_ error: Error, account: String) {
    guard !(error is CancellationError), account == (pendingAccount ?? accountIdentity) else { return }
    lastError = libraryMessage(error)
    status = "Sync needs attention"
  }
  private func observeSnapshots(_ library: NativeLibrary) {
    let listener = LibrarySnapshotListener { [weak self] in
      Task { @MainActor [weak self] in
        self?.requestRefresh()
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
      try? await Task.sleep(for: .milliseconds(100))
      while refreshAgain && !Task.isCancelled {
        refreshAgain = false
        await refresh()
      }
      snapshotRefresh = nil
    }
  }
  public func refresh() async {
    guard isReady, let native else { return }
    let epoch = accountEpoch
    do {
      let (loaded, accountPaths) = try await PopPreparation.run(on: Self.queue) {
        try native.reconcile()
        return (try native.entries(), try native.accountPaths())
      }
      guard epoch == accountEpoch, isReady else { return }
      accountBoundPaths = Set(accountPaths.map { URL(fileURLWithPath: $0).standardizedFileURL.path })
      watcher?.refresh()
      entries = loaded
      guestDocumentIDs = Set(loaded.filter { $0.role == .editor && !$0.deleted }.map(\.id))
      issues = Dictionary(uniqueKeysWithValues: loaded.compactMap { entry in entry.problem.map { (entry.id, $0) } })
      pending = loaded.filter { $0.dirty && !$0.localOnly }.count
      isSyncing = loaded.contains { $0.phase == .syncing }
      if !isSyncing && pending == 0 && issues.isEmpty && loaded.contains(where: { $0.phase == .synced }) {
        lastSuccess = Date()
      }
      hasLoaded = true
      status =
        isSyncing
        ? "Syncing…"
        : lastError != nil
          ? "Sync needs attention"
          : pending > 0
            ? "Saved here · waiting to sync" : accountIdentity.isEmpty ? "Saved on this device" : "Sync across devices"
    } catch {
      guard epoch == accountEpoch, isReady else { return }
      lastError = libraryMessage(error)
    }
  }
  private func requiredToken() async throws -> String {
    guard isReady, !accountIdentity.isEmpty, let tokenProvider else {
      throw PopFailure("Sign in to sync across devices.")
    }
    let epoch = accountEpoch
    let token = try await tokenProvider()
    guard epoch == accountEpoch, isReady else { throw CancellationError() }
    return token
  }
  public func syncNow() async {
    if let pendingAccount {
      do { try await setAccount(pendingAccount, external: !isReady) } catch {
        guard !(error is CancellationError), self.pendingAccount == pendingAccount else { return }
        lastError = libraryMessage(error)
        return
      }
    }
    if let syncTask {
      syncAgain = true
      await syncTask.value
      return
    }
    guard isReady, let native, !accountIdentity.isEmpty else {
      await refresh()
      return
    }
    let epoch = accountEpoch
    syncAgain = true
    let work = Task {
      guard epoch == accountEpoch else { return }
      isSyncing = true
      while syncAgain && !Task.isCancelled {
        syncAgain = false
        var failure: String?
        do {
          let account = accountIdentity
          let token = try await requiredToken()
          try Task.checkCancellation()
          guard epoch == accountEpoch else { return }
          try await PopPreparation.run(on: Self.queue) { try native.sync(account: account, token: token) }
        } catch { failure = libraryMessage(error) }
        guard !Task.isCancelled, epoch == accountEpoch else { return }
        lastError = failure
      }
      await refresh()
    }
    syncTask = work
    await work.value
    if !work.isCancelled, epoch == accountEpoch { syncTask = nil }
  }
  public func activated(_ active: Bool) {
    accountActive = active
    native?.setAccountActive(active: active)
    if active {
      foregroundAt = Date()
      Task {
        await syncNow()
      }
    }
  }
  public func create(template: URL, title: String, sync: Bool = false) async throws -> LibraryEntry {
    if native == nil { try await setAccount("") }
    guard let native else { throw PopFailure("The local library is unavailable.") }
    let entry = try await PopPreparation.run(on: Self.queue) {
      try native.create(template: template.path, title: title)
    }
    await refresh()
    if sync { return try await enableSync(id: entry.id) }
    return entry
  }
  @discardableResult public func enableSync(id: String) async throws -> LibraryEntry {
    let account = accountIdentity
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    let entry = try await PopPreparation.run(on: Self.queue) {
      try native.enableSync(account: account, id: id, token: token)
    }
    Task { await syncNow() }
    await refresh()
    return entry
  }
  public func enableSync(source: URL, title: String) async throws {
    guard !accountIdentity.isEmpty else { throw PopFailure("Sign in with Apple to create a cloud document.") }
    guard let native else { throw PopFailure("Sign in to sync across devices.") }
    let entry = try await PopPreparation.run(on: Self.queue) { try native.adopt(source: source.path, title: title) }
    _ = try await enableSync(id: entry.id)
  }
  public func delete(_ entry: LibraryEntry) async throws {
    guard let native else { return }
    if entry.localOnly {
      try await PopPreparation.run(on: Self.queue) { try native.deleteLocal(id: entry.id) }
    } else {
      let account = accountIdentity
      let token = try await requiredToken()
      if entry.role == .editor {
        try await PopPreparation.run(on: Self.queue) { try native.leave(account: account, id: entry.id, token: token) }
      } else {
        try await PopPreparation.run(on: Self.queue) {
          try native.deleteEverywhere(account: account, id: entry.id, token: token)
        }
      }
    }
    Task { await syncNow() }
    await refresh()
  }
  public func removeDownload(_ entry: LibraryEntry) async throws {
    guard let native else { return }
    try await PopPreparation.run(on: Self.queue) { try native.removeDownload(id: entry.id) }
    await refresh()
  }
  public func download(_ entry: LibraryEntry) async throws {
    let account = accountIdentity
    let token = try await requiredToken()
    guard let native else { return }
    _ = try await PopPreparation.run(on: Self.queue) {
      try native.download(account: account, id: entry.id, token: token)
    }
    await refresh()
  }
  @discardableResult public func localCopy(_ entry: LibraryEntry) async throws -> LibraryEntry {
    guard let native else { throw PopFailure("The library is unavailable.") }
    let copy = try await PopPreparation.run(on: Self.queue) {
      try native.localCopy(id: entry.id, title: entry.title + " copy")
    }
    await refresh()
    return copy
  }
  public func sharing(id: String, enabled: Bool) async throws -> AccessState {
    let account = accountIdentity
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    let access = try await PopPreparation.run(on: Self.queue) {
      try native.sharing(account: account, id: id, enabled: enabled, token: token)
    }
    Task { await syncNow() }
    return access
  }
  public func join(link: String) async throws -> LibraryEntry {
    let account = accountIdentity
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    let entry = try await PopPreparation.run(on: Self.queue) {
      try native.join(account: account, link: link, token: token)
    }
    Task { await syncNow() }
    await refresh()
    return entry
  }
  public func access(id: String) async throws -> AccessState {
    let account = accountIdentity
    let token = try await requiredToken()
    guard let native else { throw PopFailure("The library is unavailable.") }
    return try await PopPreparation.run(on: Self.queue) { try native.access(account: account, id: id, token: token) }
  }
  public func pauseDocument(id: String) async throws {
    guard let native else { return }
    try await PopPreparation.run(on: Self.queue) { try native.unregisterOwner(id: id) }
  }
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
