import AppKit
import ComposableArchitecture
import HitSlopFeatures

/// Holds presentation identity until AppKit reports dismissal. Store state remains authoritative.
@MainActor final class NativeAlertPresenter {
    /// Alerts only acknowledge: `ErrorAlertAction` has no cases, so dismissal is the only outcome.
    typealias Present = @MainActor (AlertState<ErrorAlertAction>, NSWindow?, @escaping @MainActor () -> Void) -> Void
    private struct Request {
        let alert: AlertState<ErrorAlertAction>
        weak var window: NSWindow?
        let isCurrent: @MainActor () -> Bool
        let dismiss: @MainActor () -> Void
    }
    private var pending: [Request] = []
    private var active: [UUID: Request] = [:]
    private let present: Present

    init(present: @escaping Present = NativeAlertPresenter.presentAlert) { self.present = present }

    func enqueue(_ alert: AlertState<ErrorAlertAction>, window: NSWindow?, isCurrent: @escaping @MainActor () -> Bool, dismiss: @escaping @MainActor () -> Void) {
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
            present(request.alert, request.window) { [weak self] in
                guard let self, let completed = self.active.removeValue(forKey: request.alert.id) else { return }
                if completed.isCurrent() { completed.dismiss() }
                Task { [weak self] in self?.drain() }
            }
        }
    }

    private static func presentAlert(_ state: AlertState<ErrorAlertAction>, window: NSWindow?, completion: @escaping @MainActor () -> Void) {
        let alert = NSAlert()
        alert.messageText = String(state: state.title)
        alert.informativeText = state.message.map { String(state: $0) } ?? ""
        for button in state.buttons { alert.addButton(withTitle: String(state: button.label)) }
        let finish: @MainActor (NSApplication.ModalResponse) -> Void = { _ in completion() }
        if let window {
            alert.beginSheetModal(for: window, completionHandler: finish)
        } else {
            finish(alert.runModal())
        }
    }
}
