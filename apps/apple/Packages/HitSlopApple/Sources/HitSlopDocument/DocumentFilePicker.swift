import AppKit

/// One upload panel per document; every WebKit request completes exactly once.
@MainActor final class DocumentFilePicker {
  typealias Presenter = @MainActor (NSWindow, Bool, Bool, @escaping ([URL]?) -> Void) -> (() -> Void)
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

  func present(in window: NSWindow, multiple: Bool, directories: Bool,
    completion: @escaping ([URL]?) -> Void
  ) {
    guard pending == nil, window.attachedSheet == nil else { completion(nil); return }
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

  private static func presentSheet(in window: NSWindow, multiple: Bool, directories: Bool,
    completion: @escaping ([URL]?) -> Void
  ) -> (() -> Void) {
    let panel = NSOpenPanel()
    panel.canChooseFiles = !directories
    panel.canChooseDirectories = directories
    panel.allowsMultipleSelection = multiple
    panel.canCreateDirectories = false
    panel.beginSheetModal(for: window) { response in
      completion(response == .OK ? panel.urls : nil)
    }
    return { panel.cancel(nil) }
  }
}
