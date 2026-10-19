import Foundation
import HitSlopCore
import HitSlopCoreBinding
import SwiftUI

private func sharingMessage(_ error: Error) -> String {
  if case SharingError.Failed(let message) = error { return message }
  return error.localizedDescription
}

/// Serial, off-main network work. The Rust client owns the transport and edits only
/// through this document's existing owner. Credentials stay opaque at this boundary.
private actor SharingDriver {
  let owner: NativeOwner
  let document: String
  var credential: String?
  var client: NativeSharing?
  init(owner: NativeOwner, document: String) {
    self.owner = owner
    self.document = document
  }
  func refresh(reconnect: Bool = false) throws -> SharingStatus? {
    let saved = try SharingCredentials.read(document)
    if saved != credential || reconnect {
      client?.pause()
      client = nil
      credential = nil
      if let saved {
        let restored = try NativeSharing.restore(owner: owner, credential: saved)
        try restored.connect()
        client = restored
        credential = saved
      }
    }
    return client?.status()
  }
  func start() throws -> SharingStatus {
    let root = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("hitSlop/SharingPreview/Seeds", isDirectory: true)
    var saved = try SharingCredentials.read(document)
    if saved == nil {
      saved = try sharingPrepare(owner: owner, root: root.path)
      try SharingCredentials.write(saved!, document: document)
    }
    let existing = try NativeSharing.restore(owner: owner, credential: saved!)
    if existing.status().phase != .preparing {
      client?.pause()
      try existing.connect()
      client = existing
      credential = saved
      return existing.status()
    }
    let staged = try sharingStage(owner: owner, credential: saved!)
    try SharingCredentials.write(staged, document: document)
    client?.pause()
    client = nil
    credential = nil
    let restored = try NativeSharing.restore(owner: owner, credential: staged)
    let published = try restored.publish(
      createKey: ProcessInfo.processInfo.environment["HITSLOP_SHARING_CREATE_KEY"] ?? "")
    // A published invitation is shown only after its recoverable credentials are saved.
    try SharingCredentials.write(published, document: document)
    try restored.connect()
    credential = published
    client = restored
    return restored.status()
  }
  func pause() {
    client?.pause()
    client = nil
    credential = nil
  }
  func stop() throws -> SharingStatus? {
    guard let client else { return nil }
    let saved = try client.stop()
    try SharingCredentials.write(saved, document: document)
    credential = saved
    return client.status()
  }
}

@MainActor public final class DocumentSharing: ObservableObject {
  @Published public private(set) var status: SharingStatus?
  @Published public private(set) var error: String?
  @Published public private(set) var busy = false
  private let driver: SharingDriver
  private var polling: Task<Void, Never>?
  private var active = true
  init(owner: DocumentOwner) {
    driver = SharingDriver(owner: owner.native, document: owner.documentID ?? "")
    resume()
  }
  public func resume() {
    guard polling == nil else { return }
    active = true
    polling = Task { [weak self] in
      while !Task.isCancelled {
        guard let self, self.active else { return }
        if !self.busy {
          do {
            let next = try await self.driver.refresh()
            guard self.active, !Task.isCancelled else { return }
            self.status = next
          } catch { if self.active { self.error = sharingMessage(error) } }
        }
        try? await Task.sleep(for: .seconds(2))
      }
    }
  }
  public func pause() async {
    active = false
    polling?.cancel()
    polling = nil
    await driver.pause()
  }
  public func create() { run { try await self.driver.start() } }
  public func retry() { run { try await self.driver.refresh(reconnect: true) } }
  public func stop() { run { try await self.driver.stop() } }
  private func run(_ operation: @escaping @MainActor () async throws -> SharingStatus?) {
    guard active, !busy else { return }
    busy = true
    error = nil
    Task {
      defer { busy = false }
      do {
        let next = try await operation()
        if active { status = next }
      } catch { if active { self.error = sharingMessage(error) } }
    }
  }

  /// Resolve the canonical local file, including an already-open document. Joining
  /// never imports a duplicate or requires an iCloud library.
  public static func join(_ link: String, candidates: [URL] = []) async throws -> URL {
    guard SharingPreview.enabled else { throw SlopFailure("Live sharing preview is disabled.") }
    let document = try sharingDocument(link: link)
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
    try await SlopPreparation.run {
      let saved = try sharingAcceptInvitation(existing: SharingCredentials.read(document), link: link)
      if let nativeOwner { _ = try NativeSharing.restore(owner: nativeOwner, credential: saved) }
      try sharingInstall(path: target.path, link: link)
      try SharingCredentials.write(saved, document: document)
    }
    live?.sharing?.retry()
    return target
  }
}
