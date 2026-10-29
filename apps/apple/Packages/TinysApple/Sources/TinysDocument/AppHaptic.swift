import SwiftUI

/// Native controls share the document preference. Background completions stay silent.
private struct AppHapticModifier<Trigger: Equatable>: ViewModifier {
  @AppStorage(HapticPreferences.enabledKey) private var enabled = true
  @Environment(\.scenePhase) private var scenePhase
  let feedback: SensoryFeedback
  let trigger: Trigger
  let condition: (Trigger, Trigger) -> Bool

  func body(content: Content) -> some View {
    #if os(iOS)
      content.sensoryFeedback(feedback, trigger: trigger) { old, new in
        enabled && scenePhase == .active && condition(old, new)
      }
    #else
      content
    #endif
  }
}

extension View {
  /// iPhone-only feedback for a deliberate native interaction or its visible result.
  public func appHaptic<Trigger: Equatable>(
    _ feedback: SensoryFeedback, trigger: Trigger,
    condition: @escaping (Trigger, Trigger) -> Bool = { _, _ in true }
  ) -> some View {
    modifier(AppHapticModifier(feedback: feedback, trigger: trigger, condition: condition))
  }
}
