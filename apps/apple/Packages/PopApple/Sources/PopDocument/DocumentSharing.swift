import Combine
import Foundation
import PopAccount
import PopCore
import PopCoreBinding
import SwiftUI

public func sharingMessage(_ error: Error) -> String {
  if case SharingError.Failed(let message) = error { return message }
  return error.localizedDescription
}

private final class SharingStatusListener: SharingListener, @unchecked Sendable {
  let update: @Sendable () -> Void
  init(_ update: @escaping @Sendable () -> Void) { self.update = update }
  func changed() { update() }
}

/// Off-main network work against the existing Rust document owner.
private actor SharingDriver {
  let owner: NativeOwner
  let document: String
  var credential: String?
  var identity: String?
  var client: NativeSharing?
  var listener: SharingStatusListener?
  var accountPhase: SharingPhase?
  private var authorizationGeneration = 0
  init(owner: NativeOwner, document: String) {
    self.owner = owner
    self.document = document
  }
  func observe(_ listener: SharingStatusListener) {
    self.listener = listener
    client?.listen(listener: listener)
  }
  /// Recovery asks the relay about this document, so it runs only for one that can have a link:
  /// a saved credential, or a library copy that may have been shared from another device.
  func refresh(token: String?, library: Bool = false, reconnect: Bool = false) async throws -> SharingStatus? {
    var saved = try SharingCredentials.read(document)
    let generation = authorizationGeneration
    var nextPhase: SharingPhase?
    if let token, saved != nil || library {
      // A lookup error is not evidence that this device stopped being creator.
      // Leave the verified phase, bearer and running client intact until success.
      let recovery = try sharingRecover(owner: owner, token: token)
      nextPhase = recovery.phase
      if let recovered = recovery.credential {
        saved = recovered
        try SharingCredentials.write(recovered, document: document)
      }
    }
    let verifiedCreator = nextPhase != nil
    var catalogPending = false
    if library {
      func pending(_ authority: CatalogAuthority) -> Bool {
        switch authority {
        case .preparing, .relay, .handoff: return true
        case .snapshot(let receipt): return receipt != nil
        case .checkpoint: return false
        }
      }
      catalogPending = pending(try await PopLibrary.shared.authority(id: document))
      if let staged = try await PopLibrary.shared.pendingAuthority(id: document) {
        catalogPending = catalogPending || pending(staged)
      }
    }
    guard generation == authorizationGeneration else {
      throw PopFailure("The sharing account changed. Refresh and retry.")
    }
    if library && !verifiedCreator {
      // A managed relay belongs to its catalog's creator. An absent bearer is
      // never permission to reinterpret it as a guest and clear its membership.
      client?.pause()
      client = nil
      credential = nil
      identity = nil
      accountPhase = catalogPending ? .paused : nil
      return status()
    }
    if catalogPending && nextPhase == .ended { nextPhase = .paused }
    accountPhase = nextPhase
    let nextIdentity = accountPhase != nil ? token : nil
    if saved != credential || reconnect || identity != nextIdentity {
      client?.pause()
      client = nil
      credential = nil
      if let saved {
        let restored = try NativeSharing.restore(
          owner: owner, credential: saved, stateDirectory: SharingLinks.stateDirectory(), identity: nextIdentity)
        if let listener { restored.listen(listener: listener) }
        client = restored
        credential = saved
        identity = nextIdentity
        if accountPhase != .paused { try restored.connect() }
      }
    }
    client?.setCreator(creator: accountPhase != nil)
    return status()
  }
  func status() -> SharingStatus? {
    if accountPhase == .ended {
      return SharingStatus(
        pauseKind: nil, phase: .ended, detail: "Sharing ended", link: nil, creator: true,
        recoveryPath: client?.status().recoveryPath)
    }
    if accountPhase == .paused {
      return SharingStatus(
        pauseKind: nil, phase: .paused,
        detail: "This live link is paused. Sign in if needed to resume or finish stopping it.", link: nil,
        creator: true, recoveryPath: client?.status().recoveryPath)
    }
    if accountPhase == .preparing {
      return SharingStatus(
        pauseKind: nil,
        phase: .preparing, detail: "Sharing upload is incomplete. Try starting the live link again.", link: nil,
        creator: true, recoveryPath: client?.status().recoveryPath)
    }
    if let client { return client.status() }
    guard let phase = accountPhase else { return nil }
    return SharingStatus(
      pauseKind: nil,
      phase: phase,
      detail: phase == .ended ? "Sharing ended" : "Sharing upload is incomplete. Try starting the live link again.",
      link: nil,
      creator: true, recoveryPath: client?.status().recoveryPath)
  }
  func start(token: String, library: Bool) async throws -> SharingStatus? {
    let generation = authorizationGeneration
    client?.pause()
    let prepared = try sharingReserve(document: document, token: token)
    let room = try prepared.room()
    var authority: CatalogAuthority?
    if library {
      if try await PopLibrary.shared.hasPendingCheckpoint(id: document) {
        await PopLibrary.shared.syncNow()
        guard try await PopLibrary.shared.hasPendingCheckpoint(id: document) == false else {
          throw PopFailure("Finish saving this pop to iCloud before starting its live link.")
        }
      }
      let committed = try await PopLibrary.shared.authority(id: document)
      let pending = try await PopLibrary.shared.pendingAuthority(id: document)
      switch committed {
      case .relay(_, let currentRoom, _) where currentRoom == room:
        authority = committed
      default:
        switch pending ?? committed {
        case .preparing(let transition, let currentRoom, let seedHash, _) where currentRoom == room:
          authority = try await PopLibrary.shared.beginRelay(
            id: document, room: room, transition: transition, seedHash: seedHash)
        case .snapshot, .checkpoint:
          let transition = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
          authority = try await PopLibrary.shared.beginRelay(id: document, room: room, transition: transition)
        case .handoff:
          throw PopFailure("Finish stopping the previous live link before starting another.")
        default:
          throw PopFailure("The catalog and live link changed. Refresh before retrying.")
        }
      }
    }
    guard generation == authorizationGeneration else {
      throw PopFailure("The sharing account changed. Refresh and retry.")
    }
    let activated = try prepared.activate(
      owner: owner, token: token, authority: authority, stateDirectory: SharingLinks.stateDirectory())
    if library {
      try await PopLibrary.shared.confirmRelay(
        id: document, room: activated.room, transition: activated.transition, seedHash: activated.seedHash)
    }
    guard generation == authorizationGeneration else {
      throw PopFailure("The sharing account changed. Refresh and retry.")
    }
    try SharingCredentials.write(activated.credential, document: document)
    return try await refresh(token: token, library: library, reconnect: true)
  }
  func accountChanged() {
    authorizationGeneration += 1
    pause()
    accountPhase = nil
  }
  func pause() {
    client?.pause()
    client = nil
    credential = nil
    identity = nil
  }
  func stop(token: String, library: Bool) async throws -> SharingStatus? {
    let generation = authorizationGeneration
    if client == nil {
      _ = try await refresh(token: token, library: library)
    }
    var initialAuthority: CatalogAuthority?
    if library { initialAuthority = try await PopLibrary.shared.authority(id: document) }
    guard generation == authorizationGeneration else {
      throw PopFailure("The sharing account changed. Refresh and retry.")
    }
    var preparing = accountPhase == .preparing
    if case .some(.preparing) = initialAuthority { preparing = true }
    if preparing {
      var expectedRoom: String?
      var transition: String?
      if case .some(.preparing(let id, let room, _, _)) = initialAuthority {
        expectedRoom = room
        transition = id
      }
      if let room = try sharingCancelPreparation(document: document, room: expectedRoom, token: token) {
        if library {
          if let transition {
            try await PopLibrary.shared.cancelPreparing(id: document, room: room, transition: transition)
          } else if case .some(.snapshot) = initialAuthority {
            // No catalog authority was published before this upload was cancelled.
          } else {
            throw PopFailure("Refresh the preparing catalog before cancelling this upload.")
          }
        }
        client?.pause()
        accountPhase = .ended
        return status()
      }
      // Activation won the conditional race. Establish its committed relay
      // binding, then use the same durable freeze/handoff path below.
      _ = try await start(token: token, library: library)
    }
    if client == nil, case .some(.snapshot(let receipt)) = initialAuthority, let receipt {
      let refreshed = try await PopLibrary.shared.handoffSnapshot(
        id: document, room: receipt.room, transition: receipt.transition, freeze: receipt.freeze)
      guard generation == authorizationGeneration else {
        throw PopFailure("The sharing account changed. Refresh and retry.")
      }
      try sharingFinalizeCatalogHandoff(owner: owner, document: document, token: token, receipt: refreshed)
      try await PopLibrary.shared.finishHandoff(id: document, receipt: refreshed)
      accountPhase = .ended
      return status()
    }
    guard let client else { throw PopFailure("Restore the live link before stopping it.") }
    var receipt: CatalogReceipt?
    var authority: CatalogAuthority?
    if library { authority = try await PopLibrary.shared.authority(id: document) }
    guard generation == authorizationGeneration else {
      throw PopFailure("The sharing account changed. Refresh and retry.")
    }
    let freeze: String
    if case .some(.snapshot(let savedReceipt)) = authority, let savedReceipt {
      receipt = savedReceipt
      freeze = savedReceipt.freeze
    } else {
      freeze = try client.freeze(token: token)
    }
    if library {
      let room: String
      let transition: String
      if let receipt {
        room = receipt.room
        transition = receipt.transition
      } else {
        switch authority {
        case .some(.relay(let id, let liveRoom, _)), .some(.handoff(let id, let liveRoom, _)):
          room = liveRoom
          transition = id
        default: throw PopFailure("Refresh the catalog before stopping this live link.")
        }
      }
      // Resume with the same frozen origin, incorporating edits accepted while
      // the earlier CloudKit receipt was in flight.
      receipt = try await PopLibrary.shared.handoffSnapshot(
        id: document, room: room, transition: transition, freeze: freeze)
    }
    guard generation == authorizationGeneration else {
      throw PopFailure("The sharing account changed. Refresh and retry.")
    }
    let saved = try client.finalize(token: token, freeze: freeze, receipt: receipt)
    if let receipt { try await PopLibrary.shared.finishHandoff(id: document, receipt: receipt) }
    try SharingCredentials.write(saved, document: document)
    credential = saved
    accountPhase = .ended
    return status()
  }
}

@MainActor public final class DocumentSharing: ObservableObject {
  @Published public private(set) var status: SharingStatus? {
    didSet {
      guard let path = status?.recoveryPath, path != discoveredRecovery else { return }
      discoveredRecovery = path
      NotificationCenter.default.post(name: .popSharingRecoveryAvailable, object: URL(fileURLWithPath: path))
      Task { [weak self] in
        do { try await SharingLinks.discoverRecoveries() } catch {
          if self?.error == nil { self?.error = sharingMessage(error) }
        }
      }
    }
  }
  private var discoveredRecovery: String?
  @Published public private(set) var error: String?
  @Published public private(set) var busy = false
  @Published public private(set) var acting = false
  private let driver: SharingDriver
  private let document: String
  private var active = true
  private var pendingRefresh = false
  private var triggers: AnyCancellable?
  init(owner: DocumentOwner) {
    document = owner.documentID ?? ""
    driver = SharingDriver(owner: owner.native, document: document)
    triggers = NotificationCenter.default.publisher(for: .popSharingAccountChanged)
      .merge(with: NotificationCenter.default.publisher(for: .popSharingForeground))
      .sink { [weak self] notification in
        let accountChanged = notification.name == .popSharingAccountChanged
        Task { @MainActor [weak self] in
          guard let self else { return }
          if accountChanged { await self.driver.accountChanged() }
          self.refresh()
        }
      }
    resume()
  }
  public func openRecovery() {
    guard let path = status?.recoveryPath else { return }
    NotificationCenter.default.post(name: .popOpenSharingRecovery, object: URL(fileURLWithPath: path))
  }
  public func resume() {
    active = true
    let listener = SharingStatusListener { [weak self] in
      Task { @MainActor [weak self] in
        guard let self, self.active, !self.busy else { return }
        self.status = await self.driver.status()
      }
    }
    Task { await driver.observe(listener) }
    refresh()
  }
  public func refresh() {
    guard !busy else {
      pendingRefresh = true
      return
    }
    run(acting: false) {
      let token = try await Self.recoveryToken()
      return try await self.driver.refresh(token: token, library: self.inLibrary)
    }
  }
  private static func recoveryToken() async throws -> String? {
    let token = await SharingAccount.shared.recoveryToken()
    if token == nil && SharingAccount.shared.signedIn {
      throw PopFailure("Your sharing account is temporarily unavailable. Try again.")
    }
    return token
  }
  private var inLibrary: Bool { PopLibrary.shared.entries.contains { $0.id == document } }
  public func pause() async {
    active = false
    await driver.pause()
  }
  /// Returns false when another sharing operation is still running.
  @discardableResult public func create() -> Bool {
    run {
      let token = try await SharingAccount.shared.token()
      return try await self.driver.start(token: token, library: self.inLibrary)
    }
  }
  public func retry() {
    run {
      let token = try await Self.recoveryToken()
      return try await self.driver.refresh(token: token, library: self.inLibrary, reconnect: true)
    }
  }
  public func stop() {
    run {
      let token = try await SharingAccount.shared.token()
      return try await self.driver.stop(token: token, library: self.inLibrary)
    }
  }
  @discardableResult
  private func run(acting: Bool = true, _ operation: @escaping @MainActor () async throws -> SharingStatus?) -> Bool {
    guard active, !busy else { return false }
    busy = true
    self.acting = acting
    error = nil
    Task {
      defer {
        busy = false
        self.acting = false
        if pendingRefresh {
          pendingRefresh = false
          refresh()
        }
      }
      do {
        let next = try await operation()
        if active { status = next }
      } catch {
        if active {
          self.error = sharingMessage(error)
          status = await driver.status()
        }
      }
    }
    return true
  }

  /// Install the authenticated resolver at host launch, before catalog downloads begin.
  public static func installLibraryResolver() {
    PopLibrary.shared.relayDownloader = { document, path, room, _ in
      guard let token = await SharingAccount.shared.recoveryToken() else {
        throw PopFailure("Sign in to the sharing account to download this live pop.")
      }
      let resolved = try await PopPreparation.run { try sharingResolve(document: document, token: token) }
      guard let resolved, resolved.room() == room else {
        throw PopFailure("The live link changed. Refresh the catalog before downloading.")
      }
      let authority = try await PopLibrary.shared.authority(id: document)
      guard let binding = try libraryRelayBinding(authority: authority), binding.room == room else {
        throw PopFailure("This pop changed authority while downloading. Refresh and retry.")
      }
      try resolved.validateAuthority(authority: authority)
      if case .relay = authority {
        try await PopLibrary.shared.confirmRelay(
          id: document, room: room, transition: binding.transition, seedHash: resolved.seedHash())
      }
      try await PopPreparation.run {
        try resolved.install(path: path, stateDirectory: SharingLinks.stateDirectory())
        try SharingCredentials.write(resolved.credential(), document: document)
      }
    }
  }

  /// Resolve the canonical local file, including an already-open document. Joining
  /// never imports a duplicate or requires an iCloud library.
  public static func join(_ link: String, candidates: [URL] = []) async throws -> URL {
    let document = try await PopPreparation.run { try sharingDocument(link: link) }
    let live = DocumentSession.sharedSession(document: document)
    let nativeOwner = live?.owner.native
    let paths = candidates + PopLibrary.shared.entries.map { URL(fileURLWithPath: $0.path) }
    let target: URL
    if let live {
      target = live.file.url
    } else if let entry = PopLibrary.shared.visibleEntries.first(where: { $0.id == document }) {
      // Keep the library's canonical location even when its file has not arrived yet.
      target = URL(fileURLWithPath: entry.path)
    } else {
      let found = try await PopPreparation.run { () throws -> URL? in
        for url in paths where FileManager.default.fileExists(atPath: url.path) {
          if (try? sharingLocalDocument(path: url.path)) == document { return url }
        }
        return nil as URL?
      }
      target =
        found
        ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("pop/SharedDocuments/\(document).pop")
    }
    // Stop the old transport before rebinding this same owner to a new room.
    await live?.sharing?.pause()
    do {
      if nativeOwner == nil {
        try await PopPreparation.run {
          if !FileManager.default.fileExists(atPath: target.path) {
            try sharingInstall(path: target.path, link: link, stateDirectory: SharingLinks.stateDirectory())
          }
        }
      }
      let opened: DocumentOwner?
      if nativeOwner == nil {
        opened = try await PopPreparation.run { try DocumentOwner(url: target) }
      } else {
        opened = nil
      }
      do {
        let bindingOwner = nativeOwner ?? opened!.native
        try await PopPreparation.run {
          let saved = try sharingJoin(owner: bindingOwner, existing: SharingCredentials.read(document), link: link)
          try SharingCredentials.write(saved, document: document)
        }
        try await opened?.close()
      } catch {
        try? await opened?.close()
        throw error
      }
    } catch {
      live?.sharing?.resume()
      throw error
    }
    live?.sharing?.resume()
    return target
  }
}
