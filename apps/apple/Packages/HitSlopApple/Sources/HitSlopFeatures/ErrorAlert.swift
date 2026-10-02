import ComposableArchitecture

/// What an alert can ask the app to do besides acknowledge it.
public enum ErrorAlertAction: Equatable, Sendable {
    /// Run the app's update check, for a document that needs a newer hitSlop.
    case checkForUpdates
}

public extension AlertState where Action == ErrorAlertAction {
    static func operationFailure(_ message: String, title: String = "Could not complete operation") -> Self {
        Self {
            TextState(title)
        } actions: {
            ButtonState(role: .cancel) { TextState("OK") }
        } message: {
            TextState(message)
        }
    }

    /// A document needs a newer hitSlop. Nothing was written; updating opens it.
    static func requiresUpdate(_ message: String) -> Self {
        Self {
            TextState("Could not open document")
        } actions: {
            ButtonState(action: .checkForUpdates) { TextState("Update hitSlop…") }
            ButtonState(role: .cancel) { TextState("Cancel") }
        } message: {
            TextState(message)
        }
    }
}
