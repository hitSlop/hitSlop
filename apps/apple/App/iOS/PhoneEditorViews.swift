import Combine
import HitSlopCore
import HitSlopDocument
import SwiftUI
import UIKit
import WebKit

struct PhoneEditorView: View {
  let session: DocumentSession
  @ObservedObject var model: PhoneDocuments
  @ObservedObject var library: SlopLibrary
  @State private var pageBackground = UIColor.systemBackground
  @State private var showSharing = false
  @State private var keyboardVisible = false
  /// How far the keyboard, with its ⌃ ⌄ ✓ toolbar, covers the bottom of the page.
  @State private var keyboardOverlap: CGFloat = 0

  /// What the page reads as `--slop-keyboard-inset`.
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
          // itself with `--slop-safe-top`/`--slop-safe-bottom`; the actions menu stays inside.
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
    .allowsHitTesting(!model.opening)
    .overlay(alignment: .topTrailing) {
      documentActions
        .buttonStyle(.plain)
        .glassEffect(.regular.interactive(), in: Circle())
        .padding(12)
    }
    // The keyboard covers the page, which keeps its size. Its own toolbar's ✓ dismisses it.
    .ignoresSafeArea(.keyboard)
    .background(PhoneNavigationGesture(blocked: model.opening || model.busy))
    .safeAreaInset(edge: .top) {
      if let notice = session.storageNotice {
        VStack(alignment: .leading, spacing: 6) {
          Text(notice).font(.caption)
          Button("Save Unshared Copy…") { model.saveUnsharedCopy() }
        }.padding().frame(maxWidth: .infinity, alignment: .leading).background(.regularMaterial)
      }
    }
    .overlay {
      if model.opening {
        ZStack {
          Color(uiColor: pageBackground).ignoresSafeArea(.container)
          ProgressView("Opening slop…").font(.callout)
        }
      }
    }
    .toolbar(.hidden, for: .navigationBar)
    .navigationBarBackButtonHidden(true)
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
                // TestFlight opens straight to the update; the download page is the fallback.
                UIApplication.shared.open(URL(string: "itms-beta://")!) { opened in
                  if !opened { UIApplication.shared.open(SlopLinks.iosDownload) }
                }
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
  /// The shell sets `--slop-keyboard-inset` and reveals the focused field.
  private func publish(_ inset: Int, to webView: WKWebView) {
    webView.evaluateJavaScript("globalThis.__slop?.keyboard?.(\(inset))", in: nil, in: .page) { _ in }
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
      Button("Back to Library", systemImage: "chevron.backward") { tap { model.close() } }
        .accessibilityIdentifier("back-to-library")
      Text(model.saveDescription)
    } label: {
      Image(systemName: "ellipsis").font(.system(size: 19, weight: .semibold))
        .frame(width: 44, height: 44)
    }
    .accessibilityLabel("Document actions").accessibilityIdentifier("document-actions")
    .disabled(model.opening || model.busy)
    .tint(.primary)
  }
}

/// Keep the system edge transition without a visible Back button. Scope the
/// delegate override to this editor and leave authored horizontal gestures alone.
private struct PhoneNavigationGesture: UIViewControllerRepresentable {
  let blocked: Bool

  func makeUIViewController(context: Context) -> Controller { Controller() }
  func updateUIViewController(_ controller: Controller, context: Context) {
    controller.blocked = blocked
    if controller.viewIfLoaded?.window != nil { controller.install() }
  }
  static func dismantleUIViewController(_ controller: Controller, coordinator: ()) { controller.restore() }

  final class Controller: UIViewController, UIGestureRecognizerDelegate {
    var blocked = false
    private weak var edgeGesture: UIGestureRecognizer?
    private weak var previousDelegate: (any UIGestureRecognizerDelegate)?
    private var previousEnabled = false
    private weak var contentGesture: UIGestureRecognizer?
    private var contentEnabled = false

    override func loadView() {
      view = UIView()
      view.isUserInteractionEnabled = false
    }
    override func viewDidAppear(_ animated: Bool) {
      super.viewDidAppear(animated)
      install()
    }
    override func viewDidDisappear(_ animated: Bool) {
      super.viewDidDisappear(animated)
      restore()
    }
    func install() {
      guard edgeGesture == nil, let navigationController,
        let gesture = navigationController.interactivePopGestureRecognizer
      else { return }
      edgeGesture = gesture
      previousDelegate = gesture.delegate
      previousEnabled = gesture.isEnabled
      gesture.delegate = self
      gesture.isEnabled = true
      contentGesture = navigationController.interactiveContentPopGestureRecognizer
      contentEnabled = contentGesture?.isEnabled ?? false
      contentGesture?.isEnabled = false
    }
    func restore() {
      if let gesture = edgeGesture, gesture.delegate === self {
        gesture.delegate = previousDelegate
        gesture.isEnabled = previousEnabled
      }
      contentGesture?.isEnabled = contentEnabled
      edgeGesture = nil
      contentGesture = nil
    }
    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
      guard !blocked, let navigationController else { return false }
      return navigationController.viewControllers.count > 1
        && navigationController.transitionCoordinator == nil
        && navigationController.presentedViewController == nil
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
      previousDelegate?.gestureRecognizer?(gestureRecognizer, shouldReceive: touch) ?? true
    }
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
    ) -> Bool {
      previousDelegate?.gestureRecognizer?(gestureRecognizer, shouldRecognizeSimultaneouslyWith: other) ?? false
    }
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer, shouldRequireFailureOf other: UIGestureRecognizer
    ) -> Bool {
      previousDelegate?.gestureRecognizer?(gestureRecognizer, shouldRequireFailureOf: other) ?? false
    }
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer, shouldBeRequiredToFailBy other: UIGestureRecognizer
    ) -> Bool {
      previousDelegate?.gestureRecognizer?(gestureRecognizer, shouldBeRequiredToFailBy: other) ?? false
    }
  }
}

private struct PhoneWebView: UIViewRepresentable {
  let webView: WKWebView
  func makeUIView(context: Context) -> WKWebView {
    webView.scrollView.contentInsetAdjustmentBehavior = .never
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
