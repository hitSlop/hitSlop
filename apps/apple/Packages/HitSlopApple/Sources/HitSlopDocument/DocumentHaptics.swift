import Foundation

/// A device preference shared by every document, never saved in a .slop file.
public enum HapticPreferences {
  public static let enabledKey = "hapticsEnabled"
  public static var isEnabled: Bool {
    UserDefaults.standard.object(forKey: enabledKey) == nil || UserDefaults.standard.bool(forKey: enabledKey)
  }
}

#if os(iOS)
  import HitSlopCoreBinding
  import UIKit

  @MainActor protocol HapticPlaying {
    func play(_ feedback: HapticFeedback, in view: UIView)
  }

  /// Generators belong to the attached editor. Reuse them for successive interactions.
  @MainActor final class DocumentHaptics: HapticPlaying {
    private weak var view: UIView?
    private var selection: UISelectionFeedbackGenerator?
    private var notification: UINotificationFeedbackGenerator?
    private var impacts: [Int: UIImpactFeedbackGenerator] = [:]

    func play(_ feedback: HapticFeedback, in view: UIView) {
      if self.view !== view {
        selection = nil
        notification = nil
        impacts.removeAll()
        self.view = view
      }
      switch feedback {
      case .selection:
        let generator = selection ?? UISelectionFeedbackGenerator(view: view)
        selection = generator
        generator.selectionChanged()
        generator.prepare()
      case .impact(let style):
        let native: UIImpactFeedbackGenerator.FeedbackStyle
        switch style {
        case .light: native = .light
        case .medium: native = .medium
        case .heavy: native = .heavy
        case .soft: native = .soft
        case .rigid: native = .rigid
        }
        let generator = impacts[native.rawValue] ?? UIImpactFeedbackGenerator(style: native, view: view)
        impacts[native.rawValue] = generator
        generator.impactOccurred()
        generator.prepare()
      case .notification(let notification):
        let native: UINotificationFeedbackGenerator.FeedbackType
        switch notification {
        case .success: native = .success
        case .warning: native = .warning
        case .error: native = .error
        }
        let generator = self.notification ?? UINotificationFeedbackGenerator(view: view)
        self.notification = generator
        generator.notificationOccurred(native)
        generator.prepare()
      }
    }
  }
#endif
