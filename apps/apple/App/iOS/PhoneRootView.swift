import PopAccount
import PopCore
import PopDocument
import PopLaunch
import PopWidgets
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
  @ObservedObject private var library = PopLibrary.shared
  @Environment(\.scenePhase) private var scenePhase
  @State private var tab = PhoneTab.recents
  @State private var path: [PhoneRoute] = []
  @State private var query = ""
  @State private var category = "Popular"
  @State private var showSettings = false
  @AppStorage(HapticPreferences.enabledKey) private var hapticsEnabled = true
  @State private var importAfterSettings = false
  @State private var showJoin = false
  @State private var showSharingAccount = false
  @State private var launchReady = false
  @State private var launchGreeting = true
  @Namespace private var zoom

  var body: some View {
    NavigationStack(path: navigationPath) {
      PhoneCatalogView(
        model: model, library: library, tab: $tab, showSettings: $showSettings, query: $query, category: $category,
        zoom: zoom
      )
      .toolbar(.hidden, for: .navigationBar)
      .navigationDestination(for: PhoneRoute.self) { route in
        switch route {
        case .template(let template):
          PhoneTemplateDetail(template: template, creating: model.busy, canSync: !library.accountIdentity.isEmpty) {
            name, sync in
            model.create(template, name: name, sync: sync)
          }
        case .document(let url):
          // The tile zooms into its document and the system's edge and swipe-down gestures
          // zoom it back; with Reduce Motion the system cross-fades instead. A page's own
          // touch handlers (`touch-action`, pointer events) keep their drags.
          PhoneDocumentScreen(url: url, model: model, library: library, showSharingAccount: $showSharingAccount)
            .navigationTransition(.zoom(sourceID: PhoneRecent.key(url), in: zoom))
        }
      }
      .overlay {
        if model.busy {
          ProgressView().padding(20).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
        }
      }
    }
    .tint(PopBrand.accent)
    .popLaunchGreeting(ready: launchReady, enabled: launchGreeting)
    .fileImporter(isPresented: $model.showImporter, allowedContentTypes: [UTType(filenameExtension: "pop") ?? .data]) {
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
          Section("Sync across devices") {
            LibrarySyncStatusView(library: library)
            Button("Sync Now") {
              model.tapped()
              Task { await model.syncNow() }
            }.disabled(library.isSyncing)
            Text("Your edits save on this device first, then sync to your other devices.").foregroundStyle(.secondary)
          }
          Section {
            Button("Open Shared Link…", systemImage: "person.2") {
              model.tapped()
              showJoin = true
            }
            Button("Account", systemImage: "person.crop.circle") {
              model.tapped()
              showSharingAccount = true
            }
            Button("Import from Files", systemImage: "square.and.arrow.down") {
              model.tapped()
              importAfterSettings = true
              showSettings = false
            }
          }
          Section("Community") {
            Link(destination: PopLinks.discord) {
              Label("Join Discord", systemImage: "bubble.left.and.bubble.right")
            }
            .accessibilityHint("Opens the dotpop Discord community")
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
      }.tint(PopBrand.accent)
    }
    .sheet(isPresented: Binding(get: { model.exported != nil }, set: { if !$0 { model.finishExport() } })) {
      if let exported = model.exported {
        PhoneExportView(url: exported) { model.finishExport(completed: $0) }
      }
    }
    .alert("dotpop", isPresented: Binding(get: { model.message != nil }, set: { if !$0 { model.message = nil } })) {
      if model.pageFailure != nil {
        Button("Reopen") { model.reopenFailedPage() }
        Button("Close", role: .cancel) { model.dismissPageFailure() }
      } else if case .failed = model.saveStatus {
        Button("Retry Save") { model.save() }
        if model.saveStatus == .failed(.full), model.session?.retainsHistory == true {
          Button("Save Unshared Copy…") { model.saveUnsharedCopy() }
        }
        Button("Keep Open", role: .cancel) { model.message = nil }
      } else {
        Button("OK", role: .cancel) { model.message = nil }
      }
    } message: {
      Text(model.message ?? "")
    }
    .appHaptic(.impact(weight: .light), trigger: model.tapFeedback)
    .appHaptic(.selection, trigger: path.last) { _, route in
      if case .template = route { true } else { false }
    }
    .appHaptic(.success, trigger: model.successFeedback)
    .appHaptic(.impact(weight: .medium), trigger: model.deletionFeedback)
    .appHaptic(.error, trigger: model.message) { _, message in message != nil }
    .appHaptic(.error, trigger: model.closeFailureFeedback)
    .appHaptic(.impact(flexibility: .soft, intensity: 0.7), trigger: model.leaveFeedback)
    .appHaptic(.impact(flexibility: .soft, intensity: 0.5), trigger: model.readyFeedback)
    .task {
      library.activated(scenePhase == .active)
      library.start()
      await model.loadTemplates()
      launchReady = true
    }
    // The document is on screen from the tap: a tile zooms into it while it opens behind.
    .onChange(of: model.presented) { _, url in
      library.editorOpen = url != nil
      if let url {
        if path.contains(where: { !$0.isDocument }) {
          // SwiftUI keeps a pushed screen by its place in the stack: a document put in place of a
          // template (a new pop, or a file or link opened over one) would take over the
          // template's screen, which was not pushed with the zoom and so has no edge-swipe or
          // swipe-down back. The stack returns to the library at once, and the document is
          // pushed from there on the next turn, as from its tile.
          var instant = Transaction()
          instant.disablesAnimations = true
          withTransaction(instant) { path = [] }
          DispatchQueue.main.async {
            if model.presented == url, path.isEmpty { path = [.document(url)] }
          }
        } else if path != [.document(url)] {
          path = [.document(url)]
        }
        tab = .recents
      } else if path.contains(where: \.isDocument) {
        path.removeAll(where: \.isDocument)
      }
    }
    .onChange(of: library.accountBoundPaths) { _, _ in model.libraryChanged() }
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
      launchGreeting = false
      if let url = activity.webpageURL { openSharingLink(url) }
    }
    .onOpenURL { url in
      launchGreeting = false
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
        // The system has committed the dismissal (a swipe past its point of no return) and the
        // page is still on screen while it animates out: the save starts now. A failed save
        // keeps the document's owner and marks its tile for Retry, never bouncing back here.
        if leavingDocument { model.leave() }
      })
  }
}
