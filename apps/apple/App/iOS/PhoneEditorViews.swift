import Combine
import PopCore
import PopDocument
import SwiftUI
import UIKit
import WebKit

/// One document on screen: its tile's artwork from the tap, then its live page, which
/// cross-fades in once ready. A document leaving keeps its page through the transition.
struct PhoneDocumentScreen: View {
  let url: URL
  @ObservedObject var model: PhoneDocuments
  @ObservedObject var library: PopLibrary
  @Binding var showSharingAccount: Bool

  var body: some View {
    let session = model.session(showing: url)
    let concealed = model.accountRequired && session != nil && session === model.session
    let revealed = concealed || session.map(model.shows) == true
    ZStack {
      if concealed {
        VStack(spacing: 16) {
          Text("Sign in to reopen this document").font(.headline)
          Text("Your file and unsaved text are retained on this device.")
          Button("Account") { showSharingAccount = true }
          Button("Retry") { Task { await library.syncNow() } }
        }.padding()
      } else if let session {
        PhoneEditorView(session: session, model: model, library: library)
      }
      if !revealed {
        PhoneOpeningSurface(url: url).transition(.opacity)
      }
    }
    .animation(.easeOut(duration: 0.25), value: revealed)
    .toolbar(.hidden, for: .navigationBar)
    .navigationBarBackButtonHidden(true)
    .onDisappear { model.documentScreenLeft() }
  }
}

/// What a document shows from the tap until its page is ready: the tile's artwork on the
/// tile's own card, so the zoom grows the tile into the page. Progress appears only when
/// opening takes a moment (a download, a slow page).
private struct PhoneOpeningSurface: View {
  let url: URL
  @State private var slow = false

  var body: some View {
    ZStack {
      PopBrand.card.ignoresSafeArea()
      VStack(spacing: 24) {
        PhoneArtworkView(url: url).frame(width: 128, height: 128)
        ProgressView().opacity(slow ? 1 : 0)
      }
    }
    .task {
      do { try await Task.sleep(for: .milliseconds(400)) } catch { return }
      withAnimation(.easeIn(duration: 0.2)) { slow = true }
    }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("Opening pop")
  }
}

struct PhoneEditorView: View {
  let session: DocumentSession
  @ObservedObject var model: PhoneDocuments
  @ObservedObject var library: PopLibrary
  @State private var pageBackground = UIColor.systemBackground
  @State private var showSharing = false
  @State private var keyboardVisible = false
  /// How far the keyboard, with its ⌃ ⌄ ✓ toolbar, covers the bottom of the page.
  @State private var keyboardOverlap: CGFloat = 0

  /// What the page reads as `--pop-keyboard-inset`.
  private var keyboardInset: Int {
    keyboardVisible ? Int(keyboardOverlap.rounded(.up)) : 0
  }

  var body: some View {
    Group {
      if let webView = session.currentWebView {
        PhoneWebView(webView: webView)
          .id(ObjectIdentifier(webView))
          .frame(maxWidth: .infinity, maxHeight: .infinity)
          // Edge to edge: the page runs under the status bar and home indicator and pads
          // itself with `--pop-safe-top`/`--pop-safe-bottom`; the actions menu stays inside.
          .ignoresSafeArea()
          .background(Color(uiColor: pageBackground).ignoresSafeArea(.container))
          .onReceive(webView.publisher(for: \.underPageBackgroundColor)) { color in
            pageBackground = color ?? .systemBackground
          }
          .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillChangeFrameNotification)) {
            note in
            guard let window = webView.window,
              let frame = note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect
            else { return }
            let keyboard = window.convert(frame, from: window.screen.coordinateSpace)
            let visible = keyboard.height > 0 && window.bounds.intersects(keyboard)
            let overlap = visible ? max(0, webView.convert(webView.bounds, to: nil).maxY - keyboard.minY) : 0
            let duration = note.userInfo?[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double ?? 0.25
            withAnimation(.easeOut(duration: duration)) {
              keyboardVisible = visible
              keyboardOverlap = overlap
            }
          }
          .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
            keyboardVisible = false
            keyboardOverlap = 0
          }
          .onChange(of: keyboardInset) { _, inset in publish(inset, to: webView) }
          .onDisappear {
            keyboardVisible = false
            keyboardOverlap = 0
          }
      }
    }
    // A page still opening or already leaving takes no touches.
    .allowsHitTesting(isOnScreen && !model.opening)
    .overlay(alignment: .topTrailing) {
      documentActions
        .buttonStyle(.plain)
        .glassEffect(.regular.interactive(), in: Circle())
        .padding(12)
    }
    // The keyboard covers the page, which keeps its size. Its own toolbar's ✓ dismisses it.
    .ignoresSafeArea(.keyboard)
    .safeAreaInset(edge: .top) {
      if let notice = session.storageNotice {
        VStack(alignment: .leading, spacing: 6) {
          Text(notice).font(.caption)
          Button("Save Unshared Copy…") { model.saveUnsharedCopy() }
        }.padding().frame(maxWidth: .infinity, alignment: .leading).background(.regularMaterial)
      }
    }
    .background {
      Color.clear.ignoresSafeArea()
        .onGeometryChange(for: CGFloat.self, of: \.safeAreaInsets.top) { _, _ in model.republishSafeArea(session) }
    }
    .sheet(isPresented: $showSharing) {
      if let sharing = session.sharing {
        NavigationStack {
          Form {
            LiveCollaborationSection(
              model: sharing,
              saveUnsharedCopy: {
                showSharing = false
                model.saveUnsharedCopy()
              },
              onUpdate: {
                UIApplication.shared.open(PopLinks.iosDownload)
              })
          }
          .navigationTitle("Share")
          .toolbar {
            ToolbarItem(placement: .confirmationAction) {
              Button("Done") { showSharing = false }
            }
          }
        }.onAppear { sharing.refresh() }
      }
    }
  }
  /// The shell sets `--pop-keyboard-inset` and reveals the focused field.
  private func publish(_ inset: Int, to webView: WKWebView) {
    webView.evaluateJavaScript("globalThis.__pop?.keyboard?.(\(inset))", in: nil, in: .page) { _ in }
  }
  /// A menu choice taps before it acts.
  private func tap(_ action: () -> Void) {
    model.tapped()
    action()
  }
  private var documentActions: some View {
    Menu {
      Button("Undo", systemImage: "arrow.uturn.backward") { tap { model.undo() } }
      Button("Redo", systemImage: "arrow.uturn.forward") { tap { model.undo(redo: true) } }
      Button("Theme", systemImage: "paintpalette") { tap { model.showTheme = true } }
      Button("Sync Now", systemImage: "arrow.triangle.2.circlepath") { tap { Task { await model.syncNow() } } }
      if session.sharing != nil {
        Button("Collaborate", systemImage: "person.2") { tap { showSharing = true } }
      }
      Button("Back Up File", systemImage: "square.and.arrow.up") { tap { model.export() } }
        .accessibilityIdentifier("export-file")
      Divider()
      Button("Back to Library", systemImage: "chevron.backward") { tap { model.leave() } }
        .accessibilityIdentifier("back-to-library")
      Text(model.saveDescription)
    } label: {
      Image(systemName: "ellipsis").font(.system(size: 19, weight: .semibold))
        .frame(width: 44, height: 44)
    }
    .accessibilityLabel("Document actions").accessibilityIdentifier("document-actions")
    .disabled(!isOnScreen || model.opening || model.busy)
    .tint(.primary)
  }
  /// The document on screen, as opposed to one animating out while it closes.
  private var isOnScreen: Bool { session === model.session }
}

private struct PhoneWebView: UIViewRepresentable {
  let webView: WKWebView
  func makeUIView(context: Context) -> WKWebView {
    webView.scrollView.contentInsetAdjustmentBehavior = .never
    // Keep WebKit from panning the fixed-height editor while the shell reveals fields.
    // This relies on WebKit registering these observers on the view at initialization;
    // requalify it on every iOS major release. Replaced views enter this adapter again.
    for name in [
      UIResponder.keyboardWillShowNotification, UIResponder.keyboardDidShowNotification,
      UIResponder.keyboardWillHideNotification, UIResponder.keyboardDidHideNotification,
      UIResponder.keyboardWillChangeFrameNotification, UIResponder.keyboardDidChangeFrameNotification,
    ] {
      NotificationCenter.default.removeObserver(webView, name: name, object: nil)
    }
    return webView
  }
  func updateUIView(_ view: WKWebView, context: Context) {}
}

struct PhoneExportView: UIViewControllerRepresentable {
  let url: URL
  let finished: (Bool) -> Void
  func makeCoordinator() -> Coordinator { Coordinator(finished) }
  func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
    let picker = UIDocumentPickerViewController(forExporting: [url], asCopy: true)
    picker.directoryURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
    picker.delegate = context.coordinator
    return picker
  }
  func updateUIViewController(_ controller: UIDocumentPickerViewController, context: Context) {}
  final class Coordinator: NSObject, UIDocumentPickerDelegate {
    let finished: (Bool) -> Void
    init(_ finished: @escaping (Bool) -> Void) { self.finished = finished }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) { finished(true) }
    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { finished(false) }
  }
}

struct PhoneThemeView: View {
  @ObservedObject var model: PhoneDocuments
  @State private var palette: [String: String] = [:]
  var body: some View {
    NavigationStack {
      Form {
        ForEach(palette.keys.sorted(), id: \.self) { token in
          ColorPicker(
            token,
            selection: Binding(
              get: { color(palette[token] ?? "#000000") },
              set: { value in
                let components = UIColor(value).resolvedColor(with: UITraitCollection(userInterfaceStyle: .light))
                var r: CGFloat = 0
                var g: CGFloat = 0
                var b: CGFloat = 0
                var a: CGFloat = 0
                guard components.getRed(&r, green: &g, blue: &b, alpha: &a) else { return }
                let hex = String(
                  format: "#%02x%02x%02x", Int((r * 255).rounded()), Int((g * 255).rounded()), Int((b * 255).rounded()))
                palette[token] = hex
                model.session?.changeTheme(.set([token: hex])) { result in
                  if case .failure(let error) = result { model.message = error.localizedDescription }
                }
              }), supportsOpacity: false)
        }
      }
      .navigationTitle("Theme")
      .toolbar { Button("Done") { model.showTheme = false } }
      .task {
        do { palette = try await model.session?.currentTheme().effective ?? [:] } catch {
          model.message = error.localizedDescription
        }
      }
    }
  }
  private func color(_ hex: String) -> Color {
    guard let value = UInt32(hex.dropFirst(), radix: 16) else { return .black }
    return Color(
      red: Double((value >> 16) & 255) / 255, green: Double((value >> 8) & 255) / 255, blue: Double(value & 255) / 255)
  }
}
