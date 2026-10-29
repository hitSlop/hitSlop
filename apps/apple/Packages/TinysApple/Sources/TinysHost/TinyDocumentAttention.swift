import AppKit
import SwiftUI
import TinysCore
import TinysDocument

/// Renderer failures, authored issues and save failures: what the window shows and reports.
extension TinyDocumentWindowController {
  public func pageSession(_ session: DocumentSession, didFail error: Error) {
    presentation.ready = false
    stopLoading()
    let context = TinyFailureContext.classify(error)
    failures.renderer(context, telemetry: telemetry)
    updatePageFailure(error.localizedDescription, context: context)
  }

  public func pageSession(_ session: DocumentSession, didReport issue: TinyPageIssue) {
    failures.issue(issue, telemetry: telemetry)
    guard guestIssue?.message != issue.message else { return }
    guestIssue = issue
    refreshIssueBadge()
  }
  public func pageSession(_ session: DocumentSession, storageFailure: TinyFailureContext) {
    telemetry.send(.breadcrumb(.save, .failed))
    telemetry.send(.failed(.save, storageFailure))
  }

  public func pageSession(_ session: DocumentSession, saveStatus: DocumentSaveStatus) {
    if saveStatus == .saved { TinyWidgetPublisher.shared.saved(session) }
    recordSaveStatus(saveStatus)
    saveAttention.shared = session.retainsHistory
    saveAttention.storageNotice = session.storageNotice
    refreshIssueBadge()
    apply(saveAttention.status(saveStatus, otherSheet: window?.attachedSheet != nil, hidden: attentionHidden))
  }
  /// The edited mark and save telemetry: one failure report until the next save.
  func recordSaveStatus(_ status: DocumentSaveStatus) {
    failures.save(status, telemetry: telemetry)
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
  /// leave the tiny running show as the issue badge instead. `SaveAttention` decides.
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
    case .saveUnsharedCopy: request(.saveUnsharedCopy)
    case .keepOpen: break
    }
  }

  /// Shows the failure screen for `message`, or removes it. The reason and classification say
  /// what the screen tells the person and what Copy details reports.
  func updatePageFailure(
    _ message: String?, context: TinyFailureContext = .init()
  ) {
    guard presentedPageError != message else { return }
    presentedPageError = message
    failedOverlay?.removeFromSuperview()
    failedOverlay = nil
    guard let message, let content = documentComposition else { return }
    sharePopover.dismiss()
    stopLoading()
    let metadata = session.file.metadata
    let screen = FailureScreen(
      notice: PageFailureNotice(
        message: message, context: context, tinyTitle: metadata.title,
        tinySlug: metadata.slug, packageFormat: session.file.packageFormat, runtimeABI: session.file.runtimeABI),
      reopen: { [weak self] in self?.request(.retry) },
      copy: { text in
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
      },
      open: { NSWorkspace.shared.open($0) })
    let overlay = NSHostingView(rootView: FailureOverlay(screen: screen))
    overlay.frame = content.bounds
    overlay.autoresizingMask = [.width, .height]
    content.addSubview(overlay)
    failedOverlay = overlay
    if presentation.requested { revealReadyWindow() }
  }
}

/// Whether the save-failure alert shows, and with what: one alert for the latest failure,
/// deferred behind an unrelated sheet, dismissed by the next save. A value, so its rules are
/// tested without a window (`SaveAttentionTests`); the window controller presents it.
struct SaveAttention: Equatable {
  /// What a button in the save-failure alert does.
  enum Action: Equatable { case retrySave, discardAndRetry, saveUnsharedCopy, keepOpen }
  struct Button: Equatable {
    let title: String
    let action: Action
  }
  struct Alert: Equatable {
    let title: String
    let message: String
    let buttons: [Button]
  }
  enum Effect: Equatable {
    case none
    case present(Alert)
    case dismiss
  }

  /// The latest failure, until the owner saves or explicitly discards the pending edits.
  private(set) var failure: SaveFailure?
  /// An unrelated sheet held the window when the failure arrived.
  private(set) var waiting = false
  /// The alert is on the window.
  private(set) var presented = false
  var shared = false
  /// What a shared document's storage tells its people (`DocumentSession.storageNotice`).
  var storageNotice: String?

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
    return .present(Self.alert(for: failure, shared: shared, storageNotice: storageNotice))
  }
  /// A sheet ended: whether a deferred failure should now try to show.
  mutating func otherSheetEnded() -> Bool {
    defer { waiting = false }
    return waiting
  }
  /// The alert ended, by a button or by the window.
  mutating func alertEnded() { presented = false }
  /// The window closed.
  mutating func windowClosed() {
    waiting = false
    presented = false
  }

  static func alert(for failure: SaveFailure, shared: Bool = false, storageNotice: String? = nil) -> Alert {
    // Unsaved work stays live; a full or stopped document offers an explicit way back to
    // the durable state.
    let buttons: [Button] =
      switch failure {
      case .invalidated:
        [
          .init(title: "Discard Unsaved Edits and Reload", action: .discardAndRetry),
          .init(title: "Keep Open", action: .keepOpen),
        ]
      case .full:
        [
          .init(title: "Retry Save", action: .retrySave),
          shared
            ? .init(title: "Save Unshared Copy…", action: .saveUnsharedCopy)
            : .init(title: "Discard Unsaved Edits", action: .discardAndRetry),
          .init(title: "Keep Open", action: .keepOpen),
        ]
      default: [.init(title: "Retry Save", action: .retrySave), .init(title: "Keep Open", action: .keepOpen)]
      }
    return Alert(
      title: failure == .invalidated ? "The document engine needs recovery" : "Changes could not be saved",
      message: failure == .full && shared
        ? storageNotice ?? failure.localizedDescription : failure.localizedDescription, buttons: buttons)
  }
}

/// The failure screen's state and buttons. The controller injects the system calls (reopening,
/// the pasteboard, the browser), so what the buttons do is tested without a window.
@MainActor @Observable final class FailureScreen {
  let notice: PageFailureNotice
  let reopen: () -> Void
  /// Technical details are open.
  var showsDetails = false
  /// Copy details has just run: its button says so for a moment.
  private(set) var copied = false
  private let copy: (String) -> Void
  private let open: (URL) -> Void
  private let confirmation: Duration
  @ObservationIgnored private var revert: Task<Void, Never>?

  init(
    notice: PageFailureNotice, reopen: @escaping () -> Void, copy: @escaping (String) -> Void,
    open: @escaping (URL) -> Void, confirmation: Duration = .milliseconds(1_500)
  ) {
    self.notice = notice
    self.reopen = reopen
    self.copy = copy
    self.open = open
    self.confirmation = confirmation
  }

  func copyDetails() {
    copy(notice.report)
    copied = true
    revert?.cancel()
    revert = Task { [weak self, confirmation] in
      try? await Task.sleep(for: confirmation)
      if !Task.isCancelled { self?.copied = false }
    }
  }
  func getHelp() { open(TinyLinks.discord) }
}

/// Covers a document whose page failed: what happened in plain words, a way back, a way to
/// report it, and the raw error out of the way.
struct FailureOverlay: View {
  @Bindable var screen: FailureScreen
  var body: some View {
    GeometryReader { proxy in
      ScrollView {
        VStack(spacing: 20) {
          VStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle").font(.largeTitle).foregroundStyle(.secondary)
              .accessibilityHidden(true)
            Text(screen.notice.title).font(.title3.weight(.semibold))
            Text(screen.notice.explanation).font(.callout).foregroundStyle(.secondary)
              .multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
          }
          VStack(spacing: 10) {
            Button("Reopen interface", action: screen.reopen).buttonStyle(.borderedProminent).controlSize(.large)
            Button(action: screen.copyDetails) {
              // The label keeps its size when it says Copied, so nothing around it moves.
              Label {
                Text(screen.copied ? "Copied" : "Copy details")
              } icon: {
                Image(systemName: screen.copied ? "checkmark" : "doc.on.doc").frame(width: 14, height: 14)
              }.frame(minWidth: 108)
            }
            .buttonStyle(.bordered)
            .onChange(of: screen.copied) { _, copied in
              if copied { AccessibilityNotification.Announcement("Copied").post() }
            }
            Button(action: screen.getHelp) {
              HStack(spacing: 3) {
                Text("Get help on Discord")
                Image(systemName: "arrow.up.right").font(.caption.weight(.semibold)).accessibilityHidden(true)
              }
            }.buttonStyle(.link).accessibilityHint("Opens Discord in your browser")
          }
          DisclosureGroup(isExpanded: $screen.showsDetails) {
            ScrollView {
              Text(screen.notice.details).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading).padding(8)
            }
            // Bounded, and smaller in a small window, so opening it still fits.
            .frame(maxHeight: min(140, max(64, proxy.size.height / 4)))
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
          } label: {
            Text("Technical details").font(.callout).foregroundStyle(.secondary)
          }
        }
        .frame(maxWidth: 420).padding(24).frame(maxWidth: .infinity, minHeight: proxy.size.height)
        // Opening the details recenters the screen: ease it rather than jump.
        .animation(.easeInOut(duration: 0.2), value: screen.showsDetails)
      }.scrollBounceBehavior(.basedOnSize)
    }.background(.regularMaterial)
  }
}
