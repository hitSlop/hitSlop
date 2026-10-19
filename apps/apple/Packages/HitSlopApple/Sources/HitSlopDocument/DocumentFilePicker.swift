#if os(macOS)
  import AppKit
#else
  import UIKit
  import UniformTypeIdentifiers
#endif

/// One upload panel per document; every WebKit request completes exactly once.
@MainActor final class DocumentFilePicker {
  typealias Presenter = @MainActor (DocumentPanelWindow, Bool, Bool, @escaping ([URL]?) -> Void) -> (() -> Void)
  private final class Request {
    let completion: ([URL]?) -> Void
    var dismiss: (() -> Void)?
    init(_ completion: @escaping ([URL]?) -> Void) { self.completion = completion }
  }
  private let presenter: Presenter
  private var pending: Request?
  var hasPendingSelection: Bool { pending != nil }

  init(presenter: @escaping Presenter = DocumentFilePicker.presentSheet) {
    self.presenter = presenter
  }

  func present(
    in window: DocumentPanelWindow, multiple: Bool, directories: Bool,
    completion: @escaping ([URL]?) -> Void
  ) {
    guard pending == nil, !window.isDocumentPanelBusy else {
      completion(nil)
      return
    }
    let request = Request(completion)
    pending = request
    request.dismiss = presenter(window, multiple, directories) { [weak self, weak request] urls in
      guard let self, let request, self.pending === request else { return }
      self.pending = nil
      request.completion(urls)
    }
  }

  func cancel() {
    guard let request = pending else { return }
    pending = nil
    request.dismiss?()
    request.completion(nil)
  }

  private static func presentSheet(
    in window: DocumentPanelWindow, multiple: Bool, directories: Bool,
    completion: @escaping ([URL]?) -> Void
  ) -> (() -> Void) {
    #if os(macOS)
      let panel = NSOpenPanel()
      panel.canChooseFiles = !directories
      panel.canChooseDirectories = directories
      panel.allowsMultipleSelection = multiple
      panel.canCreateDirectories = false
      panel.beginSheetModal(for: window) { response in
        completion(response == .OK ? panel.urls : nil)
      }
      return { panel.cancel(nil) }
    #else
      let panel = UIDocumentPickerViewController(
        forOpeningContentTypes: directories ? [.folder] : [.item], asCopy: true)
      panel.allowsMultipleSelection = multiple
      let delegate = Selection(completion)
      panel.delegate = delegate
      window.rootViewController?.present(panel, animated: true)
      // The request owns this closure until it completes or is cancelled, retaining the
      // picker's weak delegate. `asCopy` keeps WebKit reads inside this app's sandbox.
      return { [delegate] in
        _ = delegate
        panel.dismiss(animated: true)
      }
    #endif
  }

  #if os(iOS)
    private final class Selection: NSObject, UIDocumentPickerDelegate {
      let completion: ([URL]?) -> Void
      init(_ completion: @escaping ([URL]?) -> Void) { self.completion = completion }
      func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        completion(urls)
      }
      func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { completion(nil) }
    }
  #endif
}
