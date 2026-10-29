#if os(macOS)
  import AppKit
  typealias DocumentPanelWindow = NSWindow
  extension NSWindow {
    var canPresentDocumentPanel: Bool { isVisible && attachedSheet == nil }
    var isDocumentPanelBusy: Bool { attachedSheet != nil }
  }
#else
  import UIKit
  typealias DocumentPanelWindow = UIWindow
  extension UIWindow {
    var isDocumentPanelBusy: Bool { rootViewController?.presentedViewController != nil }
    var canPresentDocumentPanel: Bool {
      !isHidden && rootViewController?.presentedViewController == nil
        && UIApplication.shared.applicationState == .active
    }
  }
#endif
