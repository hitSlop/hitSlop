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
    switch saveStatus {
    case .failed(let failure):
      attentionFailure = failure
      showDocumentAttention()
    case .saved:
      attentionFailure = nil
      if let panel = documentAttention {
        window?.endSheet(panel, returnCode: .abort)
        panel.orderOut(nil)
      }
    case .saving: break
    }
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
  /// What a button in the save-failure alert does.
  private enum AttentionAction {
    case retrySave, discardAndRetry, keepOpen
  }
  /// The save-failure sheet: unsaved work is at risk, so it blocks the window. Issues that
  /// leave the slop running show as the issue badge instead.
  private func showDocumentAttention() {
    guard let message = attentionFailure?.localizedDescription else { return }
    let invalidated = attentionFailure == .invalidated
    // Unsaved work stays live; a full or stopped document offers an explicit way back to
    // the durable state.
    let actions: [(title: String, action: AttentionAction)] =
      invalidated
      ? [("Discard Unsaved Edits and Reload", .discardAndRetry), ("Keep Open", .keepOpen)]
      : attentionFailure == .full
        ? [("Retry Save", .retrySave), ("Discard Unsaved Edits", .discardAndRetry), ("Keep Open", .keepOpen)]
        : [("Retry Save", .retrySave), ("Keep Open", .keepOpen)]
    let alert = NSAlert()
    alert.messageText = invalidated ? "The document engine needs recovery" : "Changes could not be saved"
    alert.informativeText = message
    for entry in actions { alert.addButton(withTitle: entry.title) }
    guard let window, window.attachedSheet == nil else { return }
    documentAttention = alert.window as? NSPanel
    alert.beginSheetModal(for: window) { [weak self] result in
      guard let self else { return }
      self.documentAttention = nil
      // A sheet ended by the window (`.abort`) chose no button.
      let index = result.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
      if actions.indices.contains(index) { self.respond(actions[index].action) }
    }
  }
  /// Recovery is a command like any other, so it never runs beside a close or export. Its
  /// outcome returns as save status: saved dismisses the sheet, a new failure shows again.
  private func respond(_ action: AttentionAction) {
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
    stopLoading()
    if session.rendererDead, let panel = documentAttention {
      window?.endSheet(panel)
      panel.orderOut(nil)
      documentAttention = nil
      attentionFailure = nil
    }
    let overlay = NSHostingView(
      rootView: FailureOverlay(message: message, retry: { [weak self] in self?.request(.retry) }))
    overlay.frame = content.bounds
    overlay.autoresizingMask = [.width, .height]
    content.addSubview(overlay)
    failedOverlay = overlay
    if presentationRequested { revealReadyWindow() }
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
