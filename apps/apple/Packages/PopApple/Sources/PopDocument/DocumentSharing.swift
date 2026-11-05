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

/// Sharing is an access setting on the same persistent cloud document.
/// Rust owns the connection; the host supplies identity and presents its status.
@MainActor public final class DocumentSharing: ObservableObject {
  @Published public private(set) var status: SharingStatus?
  @Published public private(set) var link: String?
  @Published public private(set) var sharingEnabled = false
  @Published public private(set) var error: String?
  @Published public private(set) var busy = false
  @Published public private(set) var acting = false
  private let owner: NativeOwner
  private let file: URL
  private let document: String
  private var client: NativeSharing?
  private var listener: SharingStatusListener?
  private var active = true
  private var managed = false
  private var generation: UInt64 = 0
  private var work: Task<Void, Never>?
  private var triggers: AnyCancellable?
  private var discoveredRecovery: String?
  init(owner: DocumentOwner) {
    self.owner = owner.native
    file = owner.file.url
    document = owner.documentID ?? ""
    managed = PopLibrary.shared.entry(at: file)?.localOnly == false
    triggers = NotificationCenter.default.publisher(for: .popSharingAccountChanged)
      .merge(with: NotificationCenter.default.publisher(for: .popSharingForeground))
      .sink { [weak self] _ in Task { @MainActor [weak self] in self?.refresh() } }
    resume()
  }
  public func openRecovery() {
    guard let path = status?.recoveryPath else { return }
    NotificationCenter.default.post(name: .popOpenSharingRecovery, object: URL(fileURLWithPath: path))
  }
  public func resume() {
    active = true
    refresh()
  }
  public func pause() async throws {
    active = false
    generation += 1
    work?.cancel()
    do {
      if managed { try await PopLibrary.shared.pauseDocument(id: document) } else { client?.pause() }
    } catch {
      active = true
      throw error
    }
    client = nil
    busy = false
    acting = false
  }
  public func refresh() {
    guard active else { return }
    generation += 1
    work?.cancel()
    acting = false
    let current = generation
    work = Task {
      busy = true
      defer { if current == generation { busy = false } }
      do {
        managed = managed || PopLibrary.shared.entry(at: file)?.localOnly == false
        if managed {
          guard SharingAccount.shared.signedIn else {
            client = nil
            return
          }
          try PopLibrary.register(owner: owner)
          await PopLibrary.shared.syncNow()
          guard current == generation, active, !Task.isCancelled else { return }
          client = PopLibrary.shared.connection(id: document)
          let access = try await PopLibrary.shared.access(id: document)
          guard current == generation, active else { return }
          sharingEnabled = access.sharing
          link = access.role == .owner ? try sharingLink(access: access) : nil
        } else if let credential = try SharingCredentials.read(document) {
          if client == nil {
            let owner = owner
            let restored = try await PopPreparation.run {
              try NativeSharing.restore(
                owner: owner, credential: credential,
                stateDirectory: SharingLinks.stateDirectory())
            }
            guard current == generation, active else {
              restored.pause()
              return
            }
            client = restored
            try restored.connect()
          }
          sharingEnabled = true
        }
        observe()
        updateStatus()
        error = nil
      } catch { if current == generation { self.error = sharingMessage(error) } }
    }
  }
  private func observe() {
    let callback = SharingStatusListener { [weak self] in
      Task { @MainActor [weak self] in
        guard let self, self.active else { return }
        self.updateStatus()
      }
    }
    listener = callback
    client?.listen(listener: callback)
  }
  private func updateStatus() {
    status = client?.status()
    if !managed { link = status?.link }
    if let path = status?.recoveryPath, path != discoveredRecovery {
      discoveredRecovery = path
      NotificationCenter.default.post(name: .popSharingRecoveryAvailable, object: URL(fileURLWithPath: path))
    }
  }
  @discardableResult public func create() -> Bool { setSharing(true) }
  public func stop() { _ = setSharing(false) }
  public func retry() { refresh() }
  @discardableResult private func setSharing(_ enabled: Bool) -> Bool {
    guard active, !busy else { return false }
    generation += 1
    let current = generation
    busy = true
    acting = true
    error = nil
    work = Task {
      defer {
        if current == generation {
          busy = false
          acting = false
        }
      }
      do {
        if enabled && !managed {
          try await PopLibrary.shared.enableSync(source: file, title: file.deletingPathExtension().lastPathComponent)
          guard current == generation, active else { return }
          managed = true
        }
        let access = try await PopLibrary.shared.sharing(id: document, enabled: enabled)
        guard current == generation, active else { return }
        sharingEnabled = access.sharing
        link = access.role == .owner ? try sharingLink(access: access) : nil
        client = PopLibrary.shared.connection(id: document)
        observe()
        updateStatus()
      } catch { if current == generation { self.error = sharingMessage(error) } }
    }
    return true
  }
  public static func installLibraryServices() {
    PopLibrary.shared.start()
    SharingAccount.shared.configure()
  }

  public static func join(_ link: String, candidates: [URL] = []) async throws -> URL {
    if SharingAccount.shared.signedIn {
      let entry = try await PopLibrary.shared.join(link: link)
      return URL(fileURLWithPath: entry.path)
    }
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
    try await live?.sharing?.pause()
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
