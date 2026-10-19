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
  @State private var showLibraryName = false
  @State private var libraryName = "Quick Checklist"
  @State private var creatingLibrary = false
  @State private var showJoin = false
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
      .safeAreaInset(edge: .bottom) {
        VStack {
          if SharingPreview.enabled { Button("Join Shared Slop…") { showJoin = true } }
          Button("New synced checklist", systemImage: "plus.icloud") {
            libraryName = "Quick Checklist"
            showLibraryName = true
          }
          .buttonStyle(.bordered).disabled(!library.isReady || creatingLibrary).padding(12)
        }
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
        isCreating: model.creating != nil, isCopying: model.isCopying, isQuitting: model.isQuitting
      ) { entry in
        if entry.isSynced, let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }) {
          if let problem = document.problem, !FileManager.default.fileExists(atPath: document.path) {
            libraryError = problem
            return
          }
        }
        model.primaryAction(entry)
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
      model.start()
      library.start()
    }
    .onReceive(library.$entries) { entries in
      model.synced = entries.filter { !$0.deleted }.map { entry in
        var item = CatalogEntry(
          id: "library:" + entry.id, source: .recent(URL(fileURLWithPath: entry.path)), title: entry.title)
        item.isSynced = true
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
    .alert("iCloud", isPresented: Binding(get: { libraryError != nil }, set: { if !$0 { libraryError = nil } })) {
      Button("OK", role: .cancel) {}
    } message: {
      Text(libraryError ?? "")
    }
    .alert("Name your slop", isPresented: $showLibraryName) {
      TextField("Name", text: $libraryName)
      Button("Cancel", role: .cancel) {}
      Button("Create") { createSynced() }.disabled(libraryName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }
    .preferredColorScheme(.light)
    .sheet(isPresented: $showJoin) {
      SharingJoinView { link in
        let candidates = model.recents.compactMap { entry -> URL? in
          if case .recent(let url) = entry.source { return url }
          return nil
        }
        let url = try await DocumentSharing.join(link, candidates: candidates)
        model.primaryAction(CatalogEntry(id: url.path, source: .recent(url), title: "Shared Slop"))
      }
    }
  }
  private func createSynced() {
    guard !creatingLibrary else { return }
    let name = libraryName.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !name.isEmpty else { return }
    creatingLibrary = true
    Task {
      defer { creatingLibrary = false }
      do {
        guard let root = SlopTemplateLocation.bundledRoot else {
          throw SlopFailure("The bundled checklist is unavailable.")
        }
        let entry = try await library.create(
          template: root.appendingPathComponent("quick-checklist.slop"), title: name)
        model.select(.synced)
        var item = CatalogEntry(
          id: "library:" + entry.id, source: .recent(URL(fileURLWithPath: entry.path)), title: entry.title)
        item.isSynced = true
        model.primaryAction(item)
      } catch { libraryError = error.localizedDescription }
    }
  }
}
