import CloudKit
import CryptoKit
import Foundation
import HitSlopCoreBinding
import SwiftUI
import Synchronization

func libraryMessage(_ error: Error) -> String {
  if case LibraryError.Failed(_, let message) = error { return message }
  return error.localizedDescription
}
public typealias SyncedSlop = LibraryEntry
extension LibraryEntry: Identifiable {}
extension CloudFailure: LocalizedError { var errorDescription: String? { description } }

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

/// Native presentation of the account-scoped Rust library. No document state lives here.
@MainActor public final class SlopLibrary: ObservableObject {
  public static let shared = SlopLibrary()
  nonisolated private static let active = LibraryOwners()
  public nonisolated static func register(owner: NativeOwner) throws {
    try active.register(owner)
  }
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
  @Published public private(set) var entries: [SyncedSlop] = []
  @Published public private(set) var issues: [String: String] = [:]
  @Published public private(set) var status = "Connecting to iCloud…"
  @Published public private(set) var isSyncing = false
  @Published public private(set) var isReady = false
  @Published public private(set) var lastSuccess: Date?
  @Published public private(set) var lastError: String?
  @Published public private(set) var pending = 0
  @Published public private(set) var foregroundAt = Date()
  public var visibleEntries: [SyncedSlop] { entries.filter { !$0.deleted } }
  public var editorOpen = false
  private var sync: LibrarySync?
  private var task: Task<Void, Never>?
  private var foreground = true
  private var account = ""
  private var connection: Task<Void, Never>?
  private var accountObserver: NSObjectProtocol?
  private var syncMessage: String?
  private var native: NativeLibrary?
  private var container: CKContainer?
  private init() {}
  /// Every account's library, and earlier preview libraries beside them, live under this folder.
  public nonisolated static var managedFolder: URL {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("hitSlop/Library", isDirectory: false)
  }

  public func start() {
    guard task == nil else { return }
    accountObserver = NotificationCenter.default.addObserver(forName: .CKAccountChanged, object: nil, queue: .main) {
      _ in
      Task { @MainActor in
        self.isReady = false
        try? Self.active.activate(nil)
        await self.sync?.stop()
        self.sync = nil
        await self.request(.full)
      }
    }
    task = Task {
      if foreground { Task { await request(.full) } }
      var seconds = 0
      while !Task.isCancelled {
        if let sync, foreground {
          await refresh()
          do {
            if try await sync.changesDue() || (editorOpen && seconds % 15 == 0 && !isSyncing) {
              Task { await request(.changes) }
            }
          } catch { show(error) }
        }
        seconds += 1
        try? await Task.sleep(for: .seconds(1))
      }
    }
  }
  private func connect() async {
    if let connection {
      await connection.value
      return
    }
    let work = Task { await self.open() }
    connection = work
    await work.value
    connection = nil
  }
  private func show(_ error: Error) {
    lastError = String(describing: error)
    status = libraryMessage(error)
  }
  private func open() async {
    defer { hasLoaded = true }
    do {
      // Unsigned tools and the package test host never initialize CloudKit.
      guard Bundle.main.bundleIdentifier == "com.hitslop.app" else {
        status = "iCloud sync is available in the signed hitSlop app."
        return
      }
      let container = self.container ?? CKContainer(identifier: "iCloud.com.hitslop.app")
      self.container = container
      let accountStatus = try await container.accountStatus()
      try Task.checkCancellation()
      guard accountStatus == .available else {
        await sync?.stop()
        sync = nil
        isReady = false
        try Self.active.activate(nil)
        if accountStatus == .noAccount || accountStatus == .restricted { accountIdentity = "unavailable" }
        status = "Sign in to iCloud to sync your slops."
        return
      }
      let user = try await container.userRecordID().recordName
      try Task.checkCancellation()
      let fingerprint = SHA256.hash(data: Data(user.utf8)).map { String(format: "%02x", $0) }.joined()
      if !account.isEmpty && account != fingerprint {
        await sync?.stop()
        sync = nil
        isReady = false
        try Self.active.activate(nil)
        accountIdentity = "unavailable"
        status = "Not syncing. Your previous account’s local slops are kept on this device."
        return
      }
      if sync != nil {
        try Self.active.activate(native)
        isReady = true
        accountIdentity = account
        return
      }
      account = fingerprint
      var root = Self.managedFolder.appendingPathComponent(fingerprint, isDirectory: true)
      var zoneName = "Library"
      #if DEBUG
        // Debug builds sync with CloudKit Development. The same account maps to the same
        // folder in both environments, so keep Development's library state and files apart
        // from the Production state that TestFlight, App Store and Developer ID builds use.
        root = root.appendingPathComponent("Development", isDirectory: true)
        if let namespace = ProcessInfo.processInfo.environment["HITSLOP_LIBRARY_DIAGNOSTIC_NAMESPACE"] {
          guard !namespace.isEmpty, namespace.utf8.count <= 64,
            namespace.utf8.allSatisfy({
              (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || $0 == 45
            })
          else { throw CloudFailure("Invalid diagnostic library namespace") }
          root = root.appendingPathComponent("Diagnostics/\(namespace)", isDirectory: true)
          zoneName = "Diagnostic-" + namespace
        }
      #endif
      let libraryRoot = root
      let native = try await Task.detached(priority: .utility) {
        try NativeLibrary.open(root: libraryRoot.path, account: fingerprint)
      }.value
      try Task.checkCancellation()
      self.native = native
      try Self.active.activate(native)
      sync = LibrarySync(
        library: native, database: container.privateCloudDatabase,
        zone: CKRecordZone.ID(zoneName: zoneName, ownerName: CKCurrentUserDefaultName))
      isReady = true
      accountIdentity = account
      await refresh()
    } catch { show(error) }
  }
  public func refresh() async {
    guard let sync else { return }
    do {
      let (entries, state, syncing) = try await sync.snapshot()
      if entries != self.entries { self.entries = entries }
      issues = state.issues
      lastSuccess = state.lastSuccess
      lastError = state.lastError
      syncMessage = state.message
      pending = state.pending
      isSyncing = syncing
      hasLoaded = true
      updateStatus()
    } catch { show(error) }
  }
  private func updateStatus() {
    if isSyncing {
      status = "Syncing…"
    } else if let lastError {
      status = syncMessage ?? issues.values.sorted().first ?? lastError
    } else if !issues.isEmpty {
      status = "Some slops need attention · will retry"
    } else if pending > 0 {
      status = "Saved on this device · waiting to sync"
    } else if let lastSuccess {
      status = "Synced " + clock(lastSuccess)
    } else {
      status = "Saved on this device · waiting to sync"
    }
  }
  private func request(_ request: LibrarySync.Request, explicit: Bool = false, background: Bool = false) async {
    if request == .full || sync == nil || !isReady { await connect() }
    guard let sync, isReady else { return }
    isSyncing = true
    updateStatus()
    _ = await sync.sync(request, explicit: explicit, background: background)
    await refresh()
  }
  /// Push and background refresh use the same coalesced full reconcile.
  public func fetchInBackground() async throws {
    try Task.checkCancellation()
    await request(.full, background: true)
    try Task.checkCancellation()
    if let lastError { throw SlopFailure(lastError) }
    if !isReady { throw SlopFailure(status) }
  }
  public func receivedPush() async { await request(.full) }
  public func syncNow() async { await request(.full, explicit: true) }
  public func create(template: URL, title: String) async throws -> SyncedSlop {
    guard let sync, isReady else { throw SlopFailure("Sign in to iCloud before creating a synced slop.") }
    do {
      let entry = try await sync.create(template: template.path, title: title)
      await refresh()
      return entry
    } catch { throw SlopFailure(libraryMessage(error)) }
  }
  public func adopt(source: URL, title: String) async throws -> SyncedSlop {
    guard let sync, isReady else { throw SlopFailure("Sign in to iCloud before moving this slop.") }
    do {
      let entry = try await sync.adopt(source: source.path, title: title)
      await refresh()
      return entry
    } catch { throw SlopFailure(libraryMessage(error)) }
  }
  public func trashAdopted(source: URL, id: String) async throws {
    guard let native, isReady else { throw SlopFailure("iCloud is unavailable. Both copies were kept.") }
    do {
      try await Task.detached(priority: .utility) { try native.trashAdopted(source: source.path, id: id) }.value
    } catch { throw SlopFailure(libraryMessage(error)) }
  }
  public func delete(_ entry: SyncedSlop) async throws {
    guard let sync else { throw SlopFailure("iCloud is unavailable. Your slop is kept locally.") }
    try await sync.delete(id: entry.id)
    await refresh()
  }
  public func activated(_ active: Bool) {
    #if os(macOS)
      foreground = true
    #else
      foreground = active
    #endif
    if active {
      foregroundAt = Date()
      start()
      Task { await request(.full) }
    }
  }
  public func saveToCloud() async { await syncNow() }
  public func entry(at url: URL) -> SyncedSlop? {
    entries.first { URL(fileURLWithPath: $0.path).standardizedFileURL == url.standardizedFileURL }
  }
  private func clock(_ date: Date) -> String {
    date.formatted(date: .omitted, time: .shortened)
  }
  /// Clean and confirmed by a successful check since the app came to the front.
  public func isSynced(_ entry: SyncedSlop) -> Bool {
    guard !entry.localOnly, entry.problem == nil, issues[entry.id] == nil, !entry.dirty, isReady, lastError == nil,
      let lastSuccess
    else { return false }
    return lastSuccess >= foregroundAt
  }
  public func state(for entry: SyncedSlop) -> String {
    if entry.localOnly { return "Saved on this device" }
    if let problem = entry.problem ?? issues[entry.id] { return problem }
    if isSynced(entry) { return "Synced with iCloud" }
    if let lastSuccess { return "Saved on this device · last synced " + clock(lastSuccess) }
    return "Saved on this device · waiting to sync"
  }
}
