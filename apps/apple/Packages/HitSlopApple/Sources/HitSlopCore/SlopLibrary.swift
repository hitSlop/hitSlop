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
  @Published public private(set) var entries: [SyncedSlop] = []
  @Published public private(set) var issues: [String: String] = [:]
  @Published public private(set) var status = "Connecting to iCloud…"
  @Published public private(set) var isSyncing = false
  @Published public private(set) var isReady = false
  public var visibleEntries: [SyncedSlop] { entries.filter { !$0.deleted } }
  public var editorOpen = false
  private var transport: LibraryTransport?
  private var task: Task<Void, Never>?
  private var foreground = true
  private var account = ""
  private var connecting = false
  private var native: NativeLibrary?
  private var container: CKContainer?
  private init() {}

  public func start() {
    guard task == nil else { return }
    task = Task {
      await connect()
      var seconds = 0
      while !Task.isCancelled {
        if let transport, foreground {
          do {
            try await transport.tick()
            if editorOpen && seconds % 15 == 0 { try await transport.sync(explicit: false) }
            await refresh()
          } catch {
            status = libraryMessage(error)
            await refresh()
          }
        }
        seconds += 1
        try? await Task.sleep(for: .seconds(1))
      }
    }
  }
  private func connect() async {
    guard !connecting else { return }
    connecting = true
    defer { connecting = false }
    do {
      // Unsigned tools and the package test host never initialize CloudKit.
      guard
        Bundle.main.bundleIdentifier == "com.hitslop.app" || Bundle.main.bundleIdentifier == "com.hitslop.iospreview"
      else {
        status = "iCloud sync is available in the signed hitSlop app."
        return
      }
      let container = self.container ?? CKContainer(identifier: "iCloud.com.hitslop.syncspike")
      self.container = container
      guard try await container.accountStatus() == .available else {
        await transport?.stop()
        isReady = false
        try Self.active.activate(nil)
        status = "Sign in to iCloud to sync your slops."
        return
      }
      let user = try await container.userRecordID().recordName
      let fingerprint = SHA256.hash(data: Data(user.utf8)).map { String(format: "%02x", $0) }.joined()
      if !account.isEmpty && account != fingerprint {
        await transport?.stop()
        isReady = false
        try Self.active.activate(nil)
        status = "Not syncing. Your previous account’s local slops are kept on this device."
        return
      }
      if let transport {
        if !isReady {
          try Self.active.activate(self.native)
          try await transport.start()
          isReady = true
        }
        try await transport.sync(explicit: false)
        await refresh()
        return
      }
      account = fingerprint
      var root = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("hitSlop/Library/\(fingerprint)", isDirectory: true)
      var zoneName = "Library"
      #if DEBUG
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
      self.native = native
      try Self.active.activate(native)
      let transport = LibraryTransport(
        library: native, database: container.privateCloudDatabase,
        zone: CKRecordZone.ID(zoneName: zoneName, ownerName: CKCurrentUserDefaultName), account: fingerprint)
      self.transport = transport
      try await transport.start()
      isReady = true
      status = "iCloud connected"
      try await transport.sync(explicit: false)
      await refresh()
    } catch { status = libraryMessage(error) }
  }
  public func refresh() async {
    guard let transport else { return }
    do {
      let (entries, issues) = try await transport.snapshot()
      if entries != self.entries { self.entries = entries }
      self.issues = issues
      let stats = await transport.stats()
      if !stats.failure.isEmpty {
        isReady = false
        status = "Not syncing. \(stats.failure)"
        try Self.active.activate(nil)
      }
    } catch { status = libraryMessage(error) }
  }
  public func syncNow() async {
    guard !isSyncing else { return }
    isSyncing = true
    defer { isSyncing = false }
    if transport == nil {
      await connect()
      return
    }
    do {
      try await transport?.sync()
      status = issues.isEmpty ? "Up to date" : "Some slops need attention"
    } catch { status = libraryMessage(error) }
    await refresh()
  }
  public func create(template: URL, title: String) async throws -> SyncedSlop {
    guard let transport, isReady else { throw SlopFailure("Sign in to iCloud before creating a synced slop.") }
    let entry = try await transport.create(template: template.path, title: title)
    await refresh()
    return entry
  }
  public func delete(_ entry: SyncedSlop) async throws {
    guard let transport else { throw SlopFailure("iCloud is unavailable. Your slop is kept locally.") }
    try await transport.delete(id: entry.id)
    await refresh()
  }
  public func activated(_ active: Bool) {
    #if os(macOS)
      foreground = true
    #else
      foreground = active
    #endif
    if active {
      start()
      Task { await connect() }
    }
  }
  public func saveToCloud() async { await syncNow() }
  public func entry(at url: URL) -> SyncedSlop? {
    entries.first { URL(fileURLWithPath: $0.path).standardizedFileURL == url.standardizedFileURL }
  }
  public func state(for entry: SyncedSlop) -> String {
    if entry.localOnly { return "Saved on this device" }
    if let problem = entry.problem ?? issues[entry.id] { return problem }
    if !isReady { return "Saved locally · Not syncing" }
    return entry.dirty ? "Saved locally · Sync pending" : "Synced with iCloud"
  }
}
