import HitSlopAccount
import HitSlopCore
import HitSlopDocument
import HitSlopWidgets
import SwiftUI
import UniformTypeIdentifiers

enum PhoneRoute: Hashable {
  case template(PhoneTemplate)
  case document(URL)

  var isDocument: Bool {
    if case .document = self { return true }
    return false
  }
}

struct PhoneRootView: View {
  @ObservedObject var model: PhoneDocuments
  @ObservedObject private var library = SlopLibrary.shared
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var tab = PhoneTab.recents
  @State private var path: [PhoneRoute] = []
  @State private var query = ""
  @State private var category = "Popular"
  @State private var showSettings = false
  @AppStorage(HapticPreferences.enabledKey) private var hapticsEnabled = true
  @State private var importAfterSettings = false
  @State private var showJoin = false
  @State private var showSharingAccount = false

  var body: some View {
    NavigationStack(path: navigationPath) {
      PhoneCatalogView(
        model: model, library: library, tab: $tab, showSettings: $showSettings, query: $query, category: $category
      )
      .toolbar(.hidden, for: .navigationBar)
      .navigationDestination(for: PhoneRoute.self) { route in
        switch route {
        case .template(let template):
          PhoneTemplateDetail(template: template, creating: model.busy, canCreate: library.isReady) {
            model.create(template, name: $0)
          }
        case .document(let url):
          if let session = model.session, session.file.url == url {
            PhoneEditorView(session: session, model: model, library: library)
          }
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
          Section("Interaction") {
            Toggle("Haptic Feedback", isOn: $hapticsEnabled)
              .appHaptic(.selection, trigger: hapticsEnabled)
          }
          Section("iCloud") {
            Text(library.status)
            Button("Sync Now") { Task { await model.syncNow() } }.disabled(library.isSyncing)
            Text("Your edits save on this device first, then sync to your other devices.").foregroundStyle(.secondary)
          }
          Section {
            Button("Open Shared Link…", systemImage: "person.2") { showJoin = true }
            Button("Account", systemImage: "person.crop.circle") { showSharingAccount = true }
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
        .sheet(isPresented: $showSharingAccount) {
          NavigationStack {
            SharingAccountView()
              .toolbar { Button("Done") { showSharingAccount = false } }
          }
        }
      }.tint(PhoneStyle.accent)
    }
    .sheet(isPresented: Binding(get: { model.exported != nil }, set: { if !$0 { model.finishExport() } })) {
      if let exported = model.exported {
        PhoneExportView(url: exported) { model.finishExport(completed: $0) }
      }
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
    .appHaptic(.success, trigger: model.successFeedback)
    .appHaptic(.impact(weight: .medium), trigger: model.deletionFeedback)
    .appHaptic(.error, trigger: model.message) { _, message in message != nil }
    .task {
      library.activated(scenePhase == .active)
      library.start()
      await model.loadTemplates()
    }
    .onChange(of: model.session?.file.url) { _, url in
      library.editorOpen = url != nil
      if let url {
        withAnimation(reduceMotion ? nil : .default) { path = [.document(url)] }
        tab = .recents
      } else {
        withAnimation(reduceMotion ? nil : .default) { path.removeAll(where: \.isDocument) }
      }
    }
    .onChange(of: library.entries) { _, _ in model.libraryChanged() }
    .onChange(of: library.hasLoaded) { _, _ in model.libraryChanged() }
    .onChange(of: scenePhase) { _, phase in
      library.activated(phase == .active)
      if phase == .active {
        model.session?.sharing?.resume()
        model.widgets.foreground()
      }
      if phase == .inactive { model.saveOnInactive() }
      if phase == .background {
        PhoneAppDelegate.scheduleRefresh()
        model.saveInBackground()
      }
    }
    .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
      if let url = activity.webpageURL { openSharingLink(url) }
    }
    .onOpenURL { url in
      if openSharingLink(url) { return }
      if let id = WidgetCache.selectionID(from: url) {
        path = path.filter(\.isDocument)
        model.open(libraryID: id)
      } else if url.isFileURL {
        model.importDocument(url)
      }
    }
  }

  @discardableResult private func openSharingLink(_ url: URL) -> Bool {
    do {
      guard let link = try SharingLinks.incomingLink(url) else { return false }
      Task {
        do {
          try await model.join(link)
          showSettings = false
        } catch { model.message = sharingMessage(error) }
      }
    } catch { model.message = sharingMessage(error) }
    return true
  }

  private var navigationPath: Binding<[PhoneRoute]> {
    Binding(
      get: { path },
      set: { next in
        let leavingDocument = path.contains(where: \.isDocument) && !next.contains(where: \.isDocument)
        path = next
        if leavingDocument {
          model.close { closed in
            // A failed flush retains its owner and returns to the editor for Retry.
            if !closed, let url = model.session?.file.url {
              withAnimation(reduceMotion ? nil : .default) { path = [.document(url)] }
            }
          }
        }
      })
  }
}
