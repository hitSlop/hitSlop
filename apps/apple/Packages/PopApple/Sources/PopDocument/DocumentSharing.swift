import Combine
import Foundation
import PopAccount
import PopCore
import PopCoreBinding
import SwiftUI

public func sharingMessage(_ error: Error) -> String {
  if case SharingError.Failed(_, let message) = error { return message }
  return error.localizedDescription
}
/// Sharing is an access setting on the same persistent cloud document.
/// Rust owns the connection; the host supplies identity and presents its status.
@MainActor public final class DocumentSharing: ObservableObject {
  @Published public private(set) var status: LibraryEntry?
  @Published public private(set) var link: String?
  @Published public private(set) var access: AccessState?
  public var sharingEnabled: Bool { access?.sharing == true }
  public var joined: Bool { access?.role == .editor }
  @Published public private(set) var error: String?
  @Published public private(set) var busy = false
  @Published public private(set) var acting = false
  struct Services {
    var account: @MainActor () -> String
    var refresh: @MainActor () async throws -> AccessState?
    var setSharing: @MainActor (Bool) async throws -> AccessState
    var pause: @MainActor () async throws -> Void = {}
    var entry: @MainActor () -> LibraryEntry? = { nil }

    @MainActor static func live(owner: DocumentOwner) -> Services {
      let native = owner.native
      let file = owner.file.url
      let document = owner.documentID ?? ""
      return Services(
        account: { SharingAccount.shared.identityID ?? "" },
        refresh: {
          guard PopLibrary.shared.entry(at: file)?.localOnly == false else { return nil }
          try PopLibrary.register(owner: native)
          await PopLibrary.shared.syncNow()
          return try await PopLibrary.shared.access(id: document)
        },
        setSharing: { enabled in
          if enabled && PopLibrary.shared.entry(at: file)?.localOnly != false {
            try await PopLibrary.shared.enableSync(source: file, title: file.deletingPathExtension().lastPathComponent)
          }
          return try await PopLibrary.shared.sharing(id: document, enabled: enabled)
        },
        pause: {
          if PopLibrary.shared.entry(at: file)?.localOnly == false {
            try await PopLibrary.shared.pauseDocument(id: document)
          }
        },
        entry: { PopLibrary.shared.entries.first { $0.id == document } })
    }
  }
  private let services: Services
  private var account: String
  private var refreshRequested = false
  private var desiredSharing: Bool?
  private var active = true
  private var work: Task<Void, Never>?
  private var libraryChanges: AnyCancellable?
  private var triggers: AnyCancellable?
  convenience init(owner: DocumentOwner) { self.init(services: .live(owner: owner)) }
  init(services: Services, start: Bool = true) {
    self.services = services
    account = services.account()
    triggers = NotificationCenter.default.publisher(for: .popSharingAccountChanged)
      .merge(with: NotificationCenter.default.publisher(for: .popSharingForeground))
      .sink { @Sendable [weak self] _ in Task { @MainActor [weak self] in self?.refresh() } }
    libraryChanges = PopLibrary.shared.$entries.sink { [weak self] _ in
      Task { @MainActor [weak self] in self?.updateStatus() }
    }
    if start { resume() }
  }
  public func resume() {
    active = true
    refresh()
  }
  public func pause() async throws {
    active = false
    work?.cancel()
    work = nil
    desiredSharing = nil
    refreshRequested = false
    busy = false
    acting = false
    do { try await services.pause() } catch {
      active = true
      refresh()
      throw error
    }
  }
  public func refresh() {
    guard active else { return }
    invalidateChangedAccount()
    refreshRequested = true
    startWork()
  }
  @discardableResult private func invalidateChangedAccount() -> Bool {
    let next = services.account()
    guard next != account else { return false }
    account = next
    work?.cancel()
    work = nil
    desiredSharing = nil
    refreshRequested = false
    access = nil
    link = nil
    status = nil
    error = nil
    busy = false
    acting = false
    return true
  }
  private func isCurrent() -> Bool {
    guard active, !Task.isCancelled else { return false }
    if invalidateChangedAccount() {
      refreshRequested = true
      startWork()
      return false
    }
    return true
  }
  private func applyAccess(_ access: AccessState?) throws {
    let nextLink = try access.flatMap { $0.role == .owner ? try sharingLink(access: $0) : nil }
    self.access = access
    link = nextLink
    updateStatus()
  }
  /// Controls run serially. Routine notices request one refresh after accepted
  /// controls; they never cancel a control or supersede its committed result.
  private func startWork() {
    guard active, work == nil, desiredSharing != nil || refreshRequested else { return }
    busy = true
    acting = desiredSharing != nil
    work = Task {
      defer {
        if !Task.isCancelled {
          work = nil
          busy = false
          acting = false
        }
      }
      var controlFailure: String?
      while isCurrent() {
        if let enabled = desiredSharing {
          desiredSharing = nil
          acting = true
          do {
            let access = try await services.setSharing(enabled)
            guard isCurrent() else { return }
            try applyAccess(access)
            controlFailure = nil
            error = nil
          } catch {
            guard isCurrent() else { return }
            controlFailure = sharingMessage(error)
            self.error = controlFailure
          }
        } else if refreshRequested {
          refreshRequested = false
          acting = false
          do {
            let access = try await services.refresh()
            guard isCurrent() else { return }
            try applyAccess(access)
            error = controlFailure
          } catch {
            guard isCurrent() else { return }
            self.error = sharingMessage(error)
          }
        } else {
          return
        }
      }
    }
  }
  private func updateStatus() {
    status = active ? services.entry() : nil
  }
  @discardableResult public func create() -> Bool { setSharing(true) }
  public func stop() { _ = setSharing(false) }
  public func retry() { refresh() }
  @discardableResult private func setSharing(_ enabled: Bool) -> Bool {
    guard active else { return false }
    invalidateChangedAccount()
    desiredSharing = enabled
    acting = true
    error = nil
    startWork()
    return true
  }
  public static func installLibraryServices() {
    PopLibrary.shared.start()
    SharingAccount.shared.configure()
  }

  public static func join(_ link: String) async throws -> URL {
    try await SharingAccount.shared.signInToJoin()
    _ = try await SharingAccount.shared.token()
    let entry = try await PopLibrary.shared.join(link: link)
    return URL(fileURLWithPath: entry.path)
  }
}
