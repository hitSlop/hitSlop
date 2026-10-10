import AppKit
import HitSlopCore
import HitSlopDocument
import HitSlopFeatures
import HitSlopHost
import SwiftUI

public struct CatalogView: View {
  @Bindable var model: CatalogModel
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
        onRefresh: { model.refreshSources() }
      )
      .navigationSplitViewColumnWidth(min: 250, ideal: 280, max: 320)
      .ignoresSafeArea(.container, edges: .top)
    } detail: {
      CatalogDetail(
        section: model.filter.title, entry: model.selectedEntry,
        isCreating: model.creating != nil, isCopying: model.isCopying, isQuitting: model.isQuitting
      ) {
        model.primaryAction($0)
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
    .onAppear { model.start() }
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
    .preferredColorScheme(.light)
  }
}
