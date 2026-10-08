/// An alert the app shows: on a document's window when one is named, otherwise over the
/// app. Nothing waits for it; the window presents it and the person dismisses it.
public enum AppAlert: Equatable, Sendable {
  /// An operation failed; the alert only acknowledges it.
  case failure(String, title: String = "Could not complete operation")
  /// A document needs a newer hitSlop. Nothing was written; the alert offers the update.
  case requiresUpdate(String)
}
