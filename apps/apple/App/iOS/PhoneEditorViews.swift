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

  var body: some View {
    Group {
      if let webView = session.currentWebView {
        PhoneWebView(webView: webView)
          .id(ObjectIdentifier(webView))
          .frame(maxWidth: .infinity, maxHeight: .infinity)
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
            keyboardVisible = keyboard.height > 0 && window.bounds.intersects(keyboard)
          }
          .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
            keyboardVisible = false
          }
          .onDisappear { keyboardVisible = false }
      }
    }
    .allowsHitTesting(!model.opening)
    .overlay {
      if model.opening {
        ZStack {
          Color(uiColor: pageBackground).ignoresSafeArea(.container)
          ProgressView("Opening slop…").font(.callout)
        }
      }
    }
    .toolbar(.visible, for: .navigationBar)
    .toolbarBackground(.hidden, for: .navigationBar)
    .navigationTitle("")
    .navigationBarTitleDisplayMode(.inline)
    .toolbarRole(.editor)
    .toolbar {
      if keyboardVisible {
        ToolbarItem(placement: .topBarTrailing) {
          Button("Done") { session.currentWebView?.endEditing(true) }
            .accessibilityLabel("Dismiss keyboard")
            .accessibilityIdentifier("dismiss-keyboard")
        }
      }
      ToolbarItem(placement: .topBarTrailing) {
        Menu {
          Button("Undo", systemImage: "arrow.uturn.backward") { model.undo() }
          Button("Redo", systemImage: "arrow.uturn.forward") { model.undo(redo: true) }
          Button("Theme", systemImage: "paintpalette") { model.showTheme = true }
          Button("Sync Now", systemImage: "arrow.triangle.2.circlepath") { Task { await model.syncNow() } }
          if session.sharing != nil {
            Button("Collaborate", systemImage: "person.2") { showSharing = true }
          }
          Button("Back Up File", systemImage: "square.and.arrow.up") { model.export() }
            .accessibilityIdentifier("export-file")
          Divider()
          Text(model.saveDescription)
        } label: {
          Image(systemName: "ellipsis").font(.title3)
        }
        .accessibilityLabel("Document actions").accessibilityIdentifier("document-actions")
        .disabled(model.opening || model.busy)
      }
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
