import AppKit
import HitSlopCore
import HitSlopDocument
import SwiftUI

/// Renderer failures, authored issues and save failures: what the window shows and reports.
extension SlopDocumentWindowController {
  public func pageSession(_ session: DocumentSession, didFail error: Error) {
    isContentReady = false
    stopLoading()
    if !reportedRendererFailure {
      reportedRendererFailure = true
      let diagnostic =
        (error as? any SlopDiagnosticProviding)?.diagnostic
        ?? SlopFailureContext(reason: session.failureReason ?? .presentation)
      telemetry.send(.breadcrumb(.renderer, .failed))
      if !reportedSaveFailure || diagnostic.reason == .webContentTerminated {
        telemetry.send(.failed(.renderer, diagnostic))
      }
    }
    updatePageFailure(error.localizedDescription)
  }

  public func pageSession(_ session: DocumentSession, didReport issue: SlopPageIssue) {
    if reportedIssueKinds.insert(issue.isOperation).inserted {
      let rejection = issue.isOperation
      telemetry.send(
        .failed(
          .renderer,
          .init(
            rejection ? .rejection : .authored,
            reason: rejection ? .operationRejected : .authoredException)))
    }
    guard guestIssue?.message != issue.message else { return }
    guestIssue = issue
    refreshIssueBadge()
  }
  public func pageSession(_ session: DocumentSession, storageFailure: SlopFailureContext) {
    telemetry.send(.breadcrumb(.save, .failed))
    telemetry.send(.failed(.save, storageFailure))
  }

  public func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus) {
    recordSaveStatus(saveStatus)
    apply(saveAttention.status(saveStatus, otherSheet: window?.attachedSheet != nil, hidden: attentionHidden))
  }
  /// The edited mark and save telemetry: one failure report until the next save.
  func recordSaveStatus(_ status: DocumentSaveStatus) {
    switch status {
    case .failed:
      if !reportedSaveFailure { pageSession(session, storageFailure: .init(reason: .storage)) }
      reportedSaveFailure = true
    case .saved:
      if reportedSaveFailure { telemetry.send(.breadcrumb(.save, .recovered)) }
      reportedSaveFailure = false
    case .saving: break
    }
    window?.isDocumentEdited = status != .saved
  }
  public func pageSessionRecovered(_ session: DocumentSession) {
    if guestIssue != nil { telemetry.send(.breadcrumb(.recovery, .recovered)) }
    guestIssue = nil
    refreshIssueBadge()
    toolbar.republishControls()
  }
  private var attentionHidden: Bool { window == nil || isHiddenForClose }
  /// The save-failure sheet: unsaved work is at risk, so it blocks the window. Issues that
  /// leave the slop running show as the issue badge instead. `SaveAttention` decides.
  func showDocumentAttention() {
    apply(saveAttention.reveal(otherSheet: window?.attachedSheet != nil, hidden: attentionHidden))
  }
  /// Only an unrelated sheet defers a failure. Dismissing our own alert is final until
  /// another failure arrives, including when the person chose Keep Open.
  public func windowDidEndSheet(_ notification: Notification) {
    guard saveAttention.otherSheetEnded() else { return }
    // AppKit finishes detaching the sheet after notifying its delegate.
    DispatchQueue.main.async { [weak self] in self?.showDocumentAttention() }
  }
  func apply(_ effect: SaveAttention.Effect) {
    switch effect {
    case .none: break
    case .present(let alert): present(alert)
    case .dismiss:
      if let panel = documentAttention {
        window?.endSheet(panel, returnCode: .abort)
        panel.orderOut(nil)
      }
    }
  }
  private func present(_ content: SaveAttention.Alert) {
    guard let window else { return }
    sharePopover.dismiss()
    let alert = NSAlert()
    alert.messageText = content.title
    alert.informativeText = content.message
    for button in content.buttons { alert.addButton(withTitle: button.title) }
    documentAttention = alert.window as? NSPanel
    alert.beginSheetModal(for: window) { [weak self] result in
      guard let self else { return }
      self.documentAttention = nil
      self.saveAttention.alertEnded()
      // A sheet ended by the window (`.abort`) chose no button.
      let index = result.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
      if content.buttons.indices.contains(index) { self.respond(content.buttons[index].action) }
    }
  }
  /// Recovery is a command like any other, so it never runs beside a close or export. Its
  /// outcome returns as save status: saved dismisses the sheet, a new failure shows again.
  private func respond(_ action: SaveAttention.Action) {
    switch action {
    case .retrySave: request(.retrySave)
    case .discardAndRetry: request(.discardUnsaved)
    case .keepOpen: break
    }
  }

  func updatePageFailure(_ message: String?) {
    guard presentedPageError != message else { return }
    presentedPageError = message
    failedOverlay?.removeFromSuperview()
    failedOverlay = nil
    guard let message, let content = documentComposition else { return }
    sharePopover.dismiss()
    stopLoading()
    if session.rendererDead { apply(saveAttention.clear()) }
    let overlay = NSHostingView(
      rootView: FailureOverlay(message: message, retry: { [weak self] in self?.request(.retry) }))
    overlay.frame = content.bounds
    overlay.autoresizingMask = [.width, .height]
    content.addSubview(overlay)
    failedOverlay = overlay
    if presentationRequested { revealReadyWindow() }
  }
}

/// Whether the save-failure alert shows, and with what: one alert for the latest failure,
/// deferred behind an unrelated sheet, dismissed by the next save. A value, so its rules are
/// tested without a window (`SaveAttentionTests`); the window controller presents it.
struct SaveAttention: Equatable {
  /// What a button in the save-failure alert does.
  enum Action: Equatable { case retrySave, discardAndRetry, keepOpen }
  struct Button: Equatable {
    let title: String
    let action: Action
  }
  struct Alert: Equatable {
    let title: String
    let message: String
    let buttons: [Button]
  }
  enum Effect: Equatable { case none, present(Alert), dismiss }

  /// The latest failure, until a save or a renderer death clears it.
  private(set) var failure: SaveFailure?
  /// An unrelated sheet held the window when the failure arrived.
  private(set) var waiting = false
  /// The alert is on the window.
  private(set) var presented = false

  mutating func status(_ status: DocumentSaveStatus, otherSheet: Bool, hidden: Bool) -> Effect {
    switch status {
    case .failed(let failure):
      self.failure = failure
      return reveal(otherSheet: otherSheet, hidden: hidden)
    case .saved:
      failure = nil
      waiting = false
      return presented ? .dismiss : .none
    case .saving: return .none
    }
  }
  /// Shows the pending failure unless the alert is already up, the window is hidden for
  /// close, or an unrelated sheet holds it (then it waits for that sheet).
  mutating func reveal(otherSheet: Bool, hidden: Bool) -> Effect {
    guard !presented, !hidden, let failure else { return .none }
    guard !otherSheet else {
      waiting = true
      return .none
    }
    waiting = false
    presented = true
    return .present(Self.alert(for: failure))
  }
  /// A sheet ended: whether a deferred failure should now try to show.
  mutating func otherSheetEnded() -> Bool {
    defer { waiting = false }
    return waiting
  }
  /// The alert ended, by a button or by the window.
  mutating func alertEnded() { presented = false }
  /// The renderer died: its overlay replaces the alert.
  mutating func clear() -> Effect {
    failure = nil
    return presented ? .dismiss : .none
  }
  /// The window closed.
  mutating func windowClosed() {
    waiting = false
    presented = false
  }

  static func alert(for failure: SaveFailure) -> Alert {
    // Unsaved work stays live; a full or stopped document offers an explicit way back to
    // the durable state.
    let buttons: [Button] =
      switch failure {
      case .invalidated: [.init(title: "Discard Unsaved Edits and Reload", action: .discardAndRetry), .init(title: "Keep Open", action: .keepOpen)]
      case .full:
        [
          .init(title: "Retry Save", action: .retrySave), .init(title: "Discard Unsaved Edits", action: .discardAndRetry),
          .init(title: "Keep Open", action: .keepOpen),
        ]
      default: [.init(title: "Retry Save", action: .retrySave), .init(title: "Keep Open", action: .keepOpen)]
      }
    return Alert(
      title: failure == .invalidated ? "The document engine needs recovery" : "Changes could not be saved",
      message: failure.localizedDescription, buttons: buttons)
  }
}

struct FailureOverlay: View {
  let message: String, retry: () -> Void
  var body: some View {
    VStack(spacing: 12) {
      Image(systemName: "exclamationmark.triangle").font(.title)
      Text("This slop stopped responding").font(.headline)
      Text(message).font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
      Button("Reopen interface", action: retry).buttonStyle(.borderedProminent)
    }.padding(24).frame(maxWidth: .infinity, maxHeight: .infinity).background(.regularMaterial)
  }
}
