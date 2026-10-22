import Combine
import Foundation
import HitSlopAccount
import HitSlopCore
import HitSlopCoreBinding
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
  var client: NativeSharing?
  var listener: SharingStatusListener?
  var accountPhase: SharingPhase?
  init(owner: NativeOwner, document: String) {
    self.owner = owner
    self.document = document
  }
  func observe(_ listener: SharingStatusListener) {
    self.listener = listener
    client?.listen(listener: listener)
  }
  func refresh(token: String?, reconnect: Bool = false) throws -> SharingStatus? {
    var saved = try SharingCredentials.read(document)
    var recoveryError: Error?
    accountPhase = nil
    if let token {
      do {
        let recovery = try sharingRecover(owner: owner, token: token)
        accountPhase = recovery.phase
        if let recovered = recovery.credential {
          saved = recovered
          try SharingCredentials.write(recovered, document: document)
        }
      } catch { recoveryError = error }
    }
    if saved != credential || reconnect {
      client?.pause()
      client = nil
      credential = nil
      if let saved {
        let restored = try NativeSharing.restore(owner: owner, credential: saved)
        if let listener { restored.listen(listener: listener) }
        client = restored
        credential = saved
        try restored.connect()
      }
    }
    client?.setCreator(creator: accountPhase != nil)
    if let recoveryError { throw recoveryError }
    return status()
  }
  func status() -> SharingStatus? {
    if accountPhase == .ended {
      return SharingStatus(phase: .ended, detail: "Sharing ended", link: nil, creator: true)
    }
    if accountPhase == .preparing {
      return SharingStatus(
        phase: .preparing, detail: "Sharing upload is incomplete. Try starting the live link again.", link: nil,
        creator: true)
    }
    if let client { return client.status() }
    guard let phase = accountPhase else { return nil }
    return SharingStatus(
      phase: phase,
      detail: phase == .ended ? "Sharing ended" : "Sharing upload is incomplete. Try starting the live link again.",
      link: nil,
      creator: true)
  }
  func start(token: String) throws -> SharingStatus? {
    client?.pause()
    let published = try sharingCreate(owner: owner, token: token)
    try SharingCredentials.write(published, document: document)
    return try refresh(token: token, reconnect: true)
  }
  func pause() {
    client?.pause()
    client = nil
    credential = nil
  }
  func stop(token: String) throws -> SharingStatus? {
    if let client, accountPhase != .preparing {
      let saved = try client.stop(token: token)
      try SharingCredentials.write(saved, document: document)
      credential = saved
    } else {
      try sharingStopDocument(owner: owner, token: token)
    }
    accountPhase = .ended
    return status()
  }
}

@MainActor public final class DocumentSharing: ObservableObject {
  @Published public private(set) var status: SharingStatus?
  @Published public private(set) var error: String?
  @Published public private(set) var busy = false
  @Published public private(set) var acting = false
  private let driver: SharingDriver
  private var active = true
  private var pendingRefresh = false
  private var triggers: AnyCancellable?
  init(owner: DocumentOwner) {
    driver = SharingDriver(owner: owner.native, document: owner.documentID ?? "")
    triggers = NotificationCenter.default.publisher(for: .hitSlopSharingAccountChanged)
      .merge(with: NotificationCenter.default.publisher(for: .hitSlopSharingForeground))
      .sink { [weak self] _ in Task { @MainActor [weak self] in self?.refresh() } }
    resume()
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
      let token = try? await SharingAccount.shared.token()
      return try await self.driver.refresh(token: token)
    }
  }
  public func pause() async {
    active = false
    await driver.pause()
  }
  /// Returns false when another sharing operation is still running.
  @discardableResult public func create() -> Bool {
    run {
      let token = try await SharingAccount.shared.token()
      return try await self.driver.start(token: token)
    }
  }
  public func retry() {
    run {
      let token = try? await SharingAccount.shared.token()
      return try await self.driver.refresh(token: token, reconnect: true)
    }
  }
  public func stop() {
    run {
      let token = try await SharingAccount.shared.token()
      return try await self.driver.stop(token: token)
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

  /// Resolve the canonical local file, including an already-open document. Joining
  /// never imports a duplicate or requires an iCloud library.
  public static func join(_ link: String, candidates: [URL] = []) async throws -> URL {
    let document = try await SlopPreparation.run { try sharingDocument(link: link) }
    let live = DocumentSession.sharedSession(document: document)
    let nativeOwner = live?.owner.native
    let paths = candidates + SlopLibrary.shared.entries.map { URL(fileURLWithPath: $0.path) }
    let target: URL
    if let live {
      target = live.file.url
    } else if let entry = SlopLibrary.shared.visibleEntries.first(where: { $0.id == document }) {
      // Keep the library's canonical location even when its file has not arrived yet.
      target = URL(fileURLWithPath: entry.path)
    } else {
      let found = try await SlopPreparation.run { () throws -> URL? in
        for url in paths where FileManager.default.fileExists(atPath: url.path) {
          if (try? sharingLocalDocument(path: url.path)) == document { return url }
        }
        return nil as URL?
      }
      target =
        found
        ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("hitSlop/SharedDocuments/\(document).slop")
    }
    // Stop the old transport before rebinding this same owner to a new room.
    await live?.sharing?.pause()
    do {
      if nativeOwner == nil {
        try await SlopPreparation.run {
          if !FileManager.default.fileExists(atPath: target.path) {
            try sharingInstall(path: target.path, link: link)
          }
        }
      }
      let opened: DocumentOwner?
      if nativeOwner == nil {
        opened = try await SlopPreparation.run { try DocumentOwner(url: target) }
      } else {
        opened = nil
      }
      do {
        let bindingOwner = nativeOwner ?? opened!.native
        try await SlopPreparation.run {
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
