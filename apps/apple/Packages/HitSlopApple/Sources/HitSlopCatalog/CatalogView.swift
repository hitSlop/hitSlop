import AppKit
import HitSlopCore
import HitSlopDocument
import HitSlopFeatures
import HitSlopHost
import SwiftUI

public struct CatalogView: View {
  @Bindable var model: CatalogModel
  @ObservedObject private var library = SlopLibrary.shared
  @State private var libraryError: String?
  @State private var sharingError: String?
  @State private var showLibraryName = false
  @State private var libraryName = ""
  @State private var creatingLibrary = false
  @State private var showJoin = false
  @State private var showStorageChoice = false
  @State private var libraryTemplate: CatalogEntry?
  @State private var storageReply: CheckedContinuation<CatalogStorageChoice, Error>?
  @FocusState private var searchFocused: Bool
  @Environment(\.scenePhase) private var scenePhase

  public init(model: CatalogModel) {
    self.model = model
  }

  public var body: some View {
    NavigationSplitView {
      CatalogSidebar(filter: model.filter, categories: model.categories, recentCount: model.recents.count) {
        model.select($0)
      }
      .navigationSplitViewColumnWidth(min: 180, ideal: 200, max: 220)
    } content: {
      CatalogRail(
        title: model.filter.title, entries: model.visibleEntries,
        selectedID: $model.selectedID, query: $model.query, searchFocused: $searchFocused,
        localIssues: model.filter != .recents ? model.localIssues : [],
        onRefresh: {
          if model.filter == .synced { Task { await library.syncNow() } } else { model.refreshSources() }
        },
        onDelete: { entry in
          guard let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }) else { return }
          Task { do { try await library.delete(document) } catch { libraryError = error.localizedDescription } }
        }
      )
      .navigationSplitViewColumnWidth(min: 250, ideal: 280, max: 320)
      .ignoresSafeArea(.container, edges: .top)
    } detail: {
      CatalogDetail(
        section: model.filter.title, entry: model.selectedEntry,
        isCreating: model.creating != nil || creatingLibrary, isCopying: model.isCopying, isQuitting: model.isQuitting
      ) { entry in
        if entry.isSynced, let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }) {
          if document.problem != nil, !FileManager.default.fileExists(atPath: document.path) {
            libraryError = "This slop isn’t available on this device. Check iCloud needs attention for sync details."
            return
          }
        }
        model.primaryAction(entry)
      }
      .safeAreaInset(edge: .bottom) {
        LibrarySyncStatusView(library: library).padding()
      }
      .frame(minWidth: 460)
      .ignoresSafeArea(.container, edges: .top)
    }
    .navigationSplitViewStyle(.balanced)
    .tint(CatalogStyle.brand)
    .background {
      Button("Focus template search") { searchFocused = true }
        .keyboardShortcut("k", modifiers: .command)
        .frame(width: 1, height: 1).opacity(0)
    }
    .onAppear {
      model.chooseStorage = chooseStorage
      model.start()
      library.start()
    }
    .onReceive(NotificationCenter.default.publisher(for: .hitSlopJoinSharedSlop)) { notification in
      if let link = notification.object as? String {
        Task {
          do { try await joinSharedLink(link) } catch { sharingError = sharingMessage(error) }
        }
      } else {
        showJoin = true
      }
    }
    .onChange(of: library.entries, initial: true) { _, _ in updateLibrary() }
    .onChange(of: library.issues) { _, _ in updateLibrary() }
    .onChange(of: library.isReady) { _, _ in updateLibrary() }
    .onChange(of: library.lastSuccess) { _, _ in updateLibrary() }
    .onChange(of: library.foregroundAt) { _, _ in updateLibrary() }
    .onChange(of: library.lastError) { _, _ in updateLibrary() }
    .onChange(of: scenePhase) { _, phase in if phase == .active { model.activated() } }
    .onReceive(NotificationCenter.default.publisher(for: .hitSlopPreviewDidChange)) { notification in
      if let url = notification.object as? URL { model.artworkChanged(url) }
    }
    .alert(
      "Could not create slop",
      isPresented: Binding(
        get: { model.creationError != nil }, set: { if !$0 { model.creationError = nil } }
      )
    ) {
      Button("OK", role: .cancel) {}
    } message: {
      Text(model.creationError ?? "")
    }
    .alert(
      "Could not open shared slop",
      isPresented: Binding(get: { sharingError != nil }, set: { if !$0 { sharingError = nil } })
    ) {
      Button("OK", role: .cancel) {}
    } message: {
      Text(sharingError ?? "")
    }
    .alert("iCloud", isPresented: Binding(get: { libraryError != nil }, set: { if !$0 { libraryError = nil } })) {
      Button("OK", role: .cancel) {}
    } message: {
      Text(libraryError ?? "")
    }
    .alert("Name your slop", isPresented: $showLibraryName) {
      TextField("Name", text: $libraryName)
      Button("Cancel", role: .cancel) { finishStorage(.cancelled) }
      Button("Create") { createSynced() }.disabled(libraryName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }
    .confirmationDialog(
      "Where do you want to keep this slop?", isPresented: $showStorageChoice, titleVisibility: .visible
    ) {
      Button("iCloud — syncs with your iPhone") { showLibraryName = true }
      Button("On this Mac…") { finishStorage(.local) }
      Button("Cancel", role: .cancel) { finishStorage(.cancelled) }
    }
    .preferredColorScheme(.light)
    .sheet(isPresented: $showJoin) {
      SharingJoinView { link in try await joinSharedLink(link) }
    }
  }
  private func updateLibrary() {
    // Without a connected library there are no synced entries to stand in for its files.
    model.managedFolder = library.isReady ? SlopLibrary.managedFolder.standardizedFileURL.path : nil
    model.synced = library.entries.filter { !$0.deleted }.map { entry in
      var item = CatalogEntry(
        id: "library:" + entry.id, source: .recent(URL(fileURLWithPath: entry.path)), title: entry.title)
      item.isSynced = true
      item.inCloud = !entry.localOnly
      item.description = library.state(for: entry)
      let url = URL(fileURLWithPath: entry.path)
      item.icons = [
        CatalogArtwork(file: url, name: .icon, modifiedAt: nil, byteCount: nil, revision: entry.assetRevision)
      ]
      item.previews = [
        CatalogArtwork(file: url, name: .preview, modifiedAt: nil, byteCount: nil, revision: entry.assetRevision)
      ]
      return item
    }
  }

  private func joinSharedLink(_ link: String) async throws {
    let candidates = model.recents.compactMap { entry -> URL? in
      if case .recent(let url) = entry.source { return url }
      return nil
    }
    let url = try await DocumentSharing.join(link, candidates: candidates)
    model.primaryAction(CatalogEntry(id: url.path, source: .recent(url), title: "Shared Slop"))
  }

  private func chooseStorage(_ entry: CatalogEntry) async throws -> CatalogStorageChoice {
    guard library.isReady else { return .local }
    libraryTemplate = entry
    libraryName = entry.title
    return try await withCheckedThrowingContinuation { reply in
      storageReply = reply
      showStorageChoice = true
    }
  }

  private func finishStorage(_ choice: CatalogStorageChoice) {
    let reply = storageReply
    storageReply = nil
    libraryTemplate = nil
    reply?.resume(returning: choice)
  }

  private func createSynced() {
    guard !creatingLibrary, let template = libraryTemplate, case .local(let source) = template.source else { return }
    let name = libraryName.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !name.isEmpty else { return }
    creatingLibrary = true
    Task {
      defer { creatingLibrary = false }
      do {
        let entry = try await library.create(template: source, title: name)
        model.select(.synced)
        finishStorage(.created(URL(fileURLWithPath: entry.path)))
      } catch {
        let reply = storageReply
        storageReply = nil
        libraryTemplate = nil
        reply?.resume(throwing: error)
      }
    }
  }
}
