import AppKit
import ComposableArchitecture
import HitSlopFeatures

/// Holds presentation identity until AppKit reports dismissal. Store state remains authoritative.
@MainActor final class NativeAlertPresenter {
    private struct Request {
        let alert: AlertState<ErrorAlertAction>
        weak var window: NSWindow?
        let isCurrent: @MainActor () -> Bool
        let dismiss: @MainActor (ErrorAlertAction?) -> Void
    }
    private var pending: [Request] = []
    private var active: [UUID: Request] = [:]

    func enqueue(_ alert: AlertState<ErrorAlertAction>, window: NSWindow?, isCurrent: @escaping @MainActor () -> Bool, dismiss: @escaping @MainActor (ErrorAlertAction?) -> Void) {
        guard active[alert.id] == nil, !pending.contains(where: { $0.alert.id == alert.id }) else { return }
        pending.append(Request(alert: alert, window: window, isCurrent: isCurrent, dismiss: dismiss))
        // Never send an action from inside the observation callback that requested presentation.
        Task { [weak self] in self?.drain() }
    }

    private func drain() {
        pending.removeAll { !$0.isCurrent() }
        var index = 0
        while index < pending.count {
            let request = pending[index]
            guard !active.values.contains(where: { $0.window === request.window }) else { index += 1; continue }
            pending.remove(at: index)
            active[request.alert.id] = request
            Self.presentAlert(request.alert, window: request.window) { [weak self] action in
                guard let self, let completed = self.active.removeValue(forKey: request.alert.id) else { return }
                if completed.isCurrent() { completed.dismiss(action) }
                Task { [weak self] in self?.drain() }
            }
        }
    }

    /// Presents an alert and reports the action of the button chosen; nil acknowledges it.
    private static func presentAlert(_ state: AlertState<ErrorAlertAction>, window: NSWindow?, completion: @escaping @MainActor (ErrorAlertAction?) -> Void) {
        let alert = NSAlert()
        alert.messageText = String(state: state.title)
        alert.informativeText = state.message.map { String(state: $0) } ?? ""
        for button in state.buttons { alert.addButton(withTitle: String(state: button.label)) }
        // NSAlert numbers its buttons from `alertFirstButtonReturn` in the order added.
        let finish: @MainActor (NSApplication.ModalResponse) -> Void = { response in
            let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
            let button = state.buttons.indices.contains(index) ? state.buttons[index] : nil
            completion(button?.action.action)
        }
        if let window {
            alert.beginSheetModal(for: window, completionHandler: finish)
        } else {
            finish(alert.runModal())
        }
    }
}
