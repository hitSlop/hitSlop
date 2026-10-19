import HitSlopCore
import HitSlopDocument
import SwiftUI
import UniformTypeIdentifiers

struct PhoneRootView: View {
  @StateObject private var model = PhoneDocuments()
  @ObservedObject private var library = SlopLibrary.shared
  @Environment(\.scenePhase) private var scenePhase
  @State private var tab = PhoneTab.recents
  @State private var path: [PhoneTemplate] = []
  @State private var query = ""
  @State private var category = "Popular"
  @State private var showSettings = false
  @State private var importAfterSettings = false
  @State private var showJoin = false

  var body: some View {
    NavigationStack(path: $path) {
      Group {
        if let session = model.session {
          PhoneEditorView(session: session, model: model, library: library)
        } else {
          PhoneCatalogView(
            model: model, library: library, tab: $tab, showSettings: $showSettings, query: $query, category: $category
          )
          .toolbar(.hidden, for: .navigationBar)
        }
      }
      .navigationDestination(for: PhoneTemplate.self) { template in
        PhoneTemplateDetail(template: template, creating: model.busy, canCreate: library.isReady) {
          model.create(template, name: $0)
        }
      }
      .overlay {
        if model.busy {
          ProgressView().padding(20).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
        }
      }
    }
    .tint(PhoneStyle.accent)
    .fileImporter(isPresented: $model.showImporter, allowedContentTypes: [UTType(filenameExtension: "slop") ?? .data]) {
      result in
      switch result {
      case .success(let url): model.importDocument(url)
      case .failure(let error): model.message = error.localizedDescription
      }
    }
    .sheet(isPresented: $model.showTheme) { PhoneThemeView(model: model) }
    .sheet(
      isPresented: $showSettings,
      onDismiss: {
        if importAfterSettings {
          importAfterSettings = false
          model.showImporter = true
        }
      }
    ) {
      NavigationStack {
        Form {
          Section("iCloud") {
            Text(library.status)
            Button("Sync Now") { Task { await library.syncNow() } }.disabled(library.isSyncing)
            Text("Your edits save on this device first, then sync to your other devices.").foregroundStyle(.secondary)
          }
          Section {
            if SharingPreview.enabled {
              Button("Join Shared Slop", systemImage: "person.2") { showJoin = true }
            }
            Button("Import from Files", systemImage: "square.and.arrow.down") {
              importAfterSettings = true
              showSettings = false
            }
          }
        }
        .navigationTitle("Settings").navigationBarTitleDisplayMode(.inline)
        .toolbar { Button("Done") { showSettings = false } }
        .sheet(isPresented: $showJoin) {
          SharingJoinView { link in
            try await model.join(link)
            showSettings = false
          }
        }
      }.tint(PhoneStyle.accent)
    }
    .sheet(isPresented: Binding(get: { model.exported != nil }, set: { if !$0 { model.finishExport() } })) {
      if let exported = model.exported { PhoneExportView(url: exported, finished: model.finishExport) }
    }
    .alert("hitSlop", isPresented: Binding(get: { model.message != nil }, set: { if !$0 { model.message = nil } })) {
      if case .failed = model.saveStatus {
        Button("Retry Save") { model.save() }
        Button("Keep Open", role: .cancel) { model.message = nil }
      } else {
        Button("OK", role: .cancel) { model.message = nil }
      }
    } message: {
      Text(model.message ?? "")
    }
    .task {
      library.start()
      await model.loadTemplates()
    }
    .onChange(of: model.session != nil) { _, opened in
      library.editorOpen = opened
      if opened {
        path.removeAll()
        tab = .recents
      }
    }
    .onChange(of: library.entries) { _, _ in model.libraryChanged() }
    .onChange(of: scenePhase) { _, phase in
      library.activated(phase == .active)
      if phase == .active { model.session?.sharing?.resume() }
      if phase == .inactive { model.saveOnInactive() }
      if phase == .background { model.saveInBackground() }
    }
    .onOpenURL { model.importDocument($0) }
  }
}
