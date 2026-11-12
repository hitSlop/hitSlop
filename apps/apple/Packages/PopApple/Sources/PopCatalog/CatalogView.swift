import AppKit
import PopCore
import PopCoreBinding
import PopDocument
import PopFeatures
import PopHost
import SwiftUI

public struct CatalogView: View {
  @Bindable var model: CatalogModel
  @ObservedObject private var library = PopLibrary.shared
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

  private var navigation: some View {
    NavigationSplitView {
      CatalogSidebar(
        filter: model.filter, categories: model.categories, recentCount: model.recents.count,
        inboxCount: model.inbox.count
      ) {
        model.select($0)
      }
      .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
      .navigationSplitViewColumnWidth(min: 180, ideal: 200, max: 220)
    } content: {
      CatalogRail(
        title: model.filter.title, entries: model.visibleEntries, guestDocumentIDs: library.guestDocumentIDs,
        selectedID: $model.selectedID, query: $model.query, searchFocused: $searchFocused,
        localIssues: model.filter != .recents ? model.localIssues : [],
        recentsIssue: model.recentsIssue,
        onRefresh: {
          model.refreshSources()
          if model.filter == .synced || model.filter == .inbox { Task { await library.syncNow() } }
        },
        onDelete: { entry in
          guard let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }) else { return }
          Task { do { try await library.delete(document) } catch { libraryError = libraryMessage(error) } }
        },
        onRemoveDownload: { entry in libraryAction(entry) { try await library.removeDownload($0) } },
        onLocalCopy: { entry in libraryAction(entry) { _ = try await library.localCopy($0) } },
        onEnableSync: { entry in libraryAction(entry) { _ = try await library.enableSync(id: $0.id) } },
        onRemoveRecent: { model.removeRecent($0) }
      )
      .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
      .navigationSplitViewColumnWidth(min: 250, ideal: 280, max: 320)
      .ignoresSafeArea(.container, edges: .top)
    } detail: {
      CatalogDetail(
        section: model.filter.title, entry: model.selectedEntry,
        isCreating: model.creating != nil || creatingLibrary,
        isCopying: (model.isCopying && model.selectedEntry?.id == model.creating?.id)
          || (creatingLibrary && model.selectedEntry?.id == libraryTemplate?.id),
        isQuitting: model.isQuitting,
        removeRecent: { model.removeRecent($0) }
      ) { entry in
        if entry.isManaged, let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }) {
          if document.problem != nil, !FileManager.default.fileExists(atPath: document.path) {
            libraryError = "This pop isn’t available on this device. Check Sync needs attention for sync details."
            return
          }
        }
        if entry.isManaged, let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }),
          !document.downloaded
        {
          libraryAction(entry) { document in
            try await library.download(document)
            model.primaryAction(entry)
          }
        } else {
          model.primaryAction(entry)
        }
      }
      .safeAreaInset(edge: .bottom) {
        LibrarySyncStatusView(library: library).padding()
      }
      .frame(minWidth: 460)
      .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
      .ignoresSafeArea(.container, edges: .top)
    }
  }

  private var observedCatalog: some View {
    navigation
      .navigationSplitViewStyle(.balanced)
      .tint(PopBrand.accent)
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
      .onReceive(NotificationCenter.default.publisher(for: .popJoinSharedPop)) { notification in
        if let link = notification.object as? String {
          Task {
            do { try await joinSharedLink(link) } catch is CancellationError {
              // Cancelling sign-in drops this invitation.
            } catch { sharingError = sharingMessage(error) }
          }
        } else {
          showJoin = true
        }
      }
      .onChange(of: library.accountBoundPaths) { _, _ in updateLibrary() }
      .onChange(of: library.entries, initial: true) { _, _ in updateLibrary() }
      .onChange(of: library.issues) { _, _ in updateLibrary() }
      .onChange(of: library.isReady) { _, _ in updateLibrary() }
      .onChange(of: library.lastSuccess) { _, _ in updateLibrary() }
      .onChange(of: library.foregroundAt) { _, _ in updateLibrary() }
      .onChange(of: library.lastError) { _, _ in updateLibrary() }
      .onChange(of: scenePhase) { _, phase in if phase == .active { model.activated() } }
      .onReceive(NotificationCenter.default.publisher(for: .popPreviewDidChange)) { notification in
        if let url = notification.object as? URL { model.artworkChanged(url) }
      }
  }

  public var body: some View {
    observedCatalog
      .alert(
        "Could not create pop",
        isPresented: Binding(
          get: { model.creationError != nil }, set: { if !$0 { model.creationError = nil } }
        )
      ) {
        Button("OK", role: .cancel) {}
      } message: {
        Text(model.creationError ?? "")
      }
      .alert(
        "Could not open shared pop",
        isPresented: Binding(get: { sharingError != nil }, set: { if !$0 { sharingError = nil } })
      ) {
        Button("OK", role: .cancel) {}
      } message: {
        Text(sharingError ?? "")
      }
      .alert(
        "Sync across devices",
        isPresented: Binding(get: { libraryError != nil }, set: { if !$0 { libraryError = nil } })
      ) {
        Button("OK", role: .cancel) {}
      } message: {
        Text(libraryError ?? "")
      }
      .alert("Name your pop", isPresented: $showLibraryName) {
        TextField("Name", text: $libraryName)
        Button("Cancel", role: .cancel) { finishStorage(.cancelled) }
        Button("Create") { createSynced() }.disabled(
          libraryName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
      }
      .confirmationDialog(
        "Where do you want to keep this pop?", isPresented: $showStorageChoice, titleVisibility: .visible
      ) {
        Button("This device…") { finishStorage(.local) }
        Button("Sync across devices") { showLibraryName = true }.disabled(library.accountIdentity.isEmpty)
        Button("Cancel", role: .cancel) { finishStorage(.cancelled) }
      }
      .preferredColorScheme(.light)
      .sheet(isPresented: $showJoin) {
        SharingJoinView { link in try await joinSharedLink(link) }
      }
  }
  private func libraryAction(_ entry: CatalogEntry, _ action: @escaping @MainActor (LibraryEntry) async throws -> Void)
  {
    guard let document = library.visibleEntries.first(where: { "library:" + $0.id == entry.id }) else { return }
    Task { do { try await action(document) } catch { libraryError = libraryMessage(error) } }
  }
  private func updateLibrary() {
    // Without a connected library there are no synced entries to stand in for its files.
    model.managedFolder = PopLibrary.managedFolder.standardizedFileURL.path
    model.accountBoundPaths = library.accountBoundPaths
    model.synced = library.entries.filter { !$0.deleted }.map { entry in
      var item = CatalogEntry(
        id: "library:" + entry.id, source: .recent(URL(fileURLWithPath: entry.path)), title: entry.title)
      item.isManaged = true
      item.inCloud = !entry.localOnly
      item.inbox = entry.inbox
      item.savedDocumentID = entry.id
      item.documentPresentation = entry.catalogPresentation
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
    let url = try await DocumentSharing.join(link)
    model.primaryAction(CatalogEntry(id: url.path, source: .recent(url), title: "Shared Pop"))
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
        let entry = try await library.create(template: source, title: name, sync: true)
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
