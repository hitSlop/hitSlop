import AppKit
import HitSlopCore
import HitSlopDocument
import SwiftUI

/// A real AppKit button gives both share surfaces a stable anchor in the hover panel.
struct SlopShareButton: NSViewRepresentable {
  let anchor: (NSView) -> Void
  let action: () -> Void

  func makeNSView(context: Context) -> SlopShareAnchorButton { SlopShareAnchorButton() }
  func updateNSView(_ button: SlopShareAnchorButton, context: Context) {
    button.clicked = action
    button.isEnabled = context.environment.isEnabled
    anchor(button)
  }
}

@MainActor final class SlopShareAnchorButton: NSButton {
  var clicked: () -> Void = {}
  init() {
    super.init(frame: .zero)
    title = ""
    image = NSImage(systemSymbolName: "square.and.arrow.up", accessibilityDescription: "Share")
    imagePosition = .imageOnly
    isBordered = false
    toolTip = "Share & Save…"
    setAccessibilityLabel("Share & Save")
    target = self
    action = #selector(share)
  }
  required init?(coder: NSCoder) { nil }
  @objc private func share() { clicked() }
}

@MainActor final class SlopSharePopover: NSObject, ObservableObject, NSPopoverDelegate {
  let sharing: DocumentSharing?
  private var collaborationInteracting = false
  let canExport: Bool
  @Published var commandsEnabled = true
  @Published private(set) var preparing: String?
  private let filename: String
  private let holdToolbar: (Bool) -> Void
  private let select: (SlopDocumentCommand) -> Void
  private let popover = NSPopover()
  private var picking = false

  var isShown: Bool { popover.isShown }
  var window: NSWindow? { popover.contentViewController?.view.window }

  init(
    filename: String, canExport: Bool, sharing: DocumentSharing? = nil, holdToolbar: @escaping (Bool) -> Void,
    select: @escaping (SlopDocumentCommand) -> Void
  ) {
    self.sharing = sharing
    self.filename = filename
    self.canExport = canExport
    self.holdToolbar = holdToolbar
    self.select = select
    super.init()
    popover.behavior = .transient
    // The native picker replaces this surface immediately after preparation.
    popover.animates = false
    popover.delegate = self
  }

  func toggle(at anchor: NSView) {
    guard preparing == nil, !picking, anchor.window?.isVisible == true else { return }
    if isShown {
      dismiss()
      return
    }
    sharing?.refresh()
    popover.contentViewController = NSHostingController(rootView: SlopShareView(model: self, filename: filename))
    holdToolbar(true)
    popover.show(relativeTo: anchor.bounds, of: anchor, preferredEdge: .minY)
    if !isShown {
      popover.contentViewController = nil
      holdToolbar(false)
    }
  }

  func choose(_ command: SlopDocumentCommand) {
    guard commandsEnabled, canDismiss, isShown else { return }
    select(command)
  }

  func beginPreparing(_ label: String) {
    preparing = label
  }

  func handOffToPicker() {
    picking = true
    preparing = nil
    popover.close()
  }

  func pickerDismissed() {
    picking = false
    holdToolbar(false)
  }

  func dismiss() {
    guard canDismiss else { return }
    popover.close()
    preparing = nil
    if !picking { holdToolbar(false) }
  }

  var canDismiss: Bool { preparing == nil && sharing?.acting != true && !collaborationInteracting }
  func setCollaborationInteracting(_ value: Bool) { collaborationInteracting = value }
  func popoverShouldClose(_ popover: NSPopover) -> Bool { canDismiss }
  func popoverDidClose(_ notification: Notification) {
    collaborationInteracting = false
    if !picking { holdToolbar(false) }
    // Break the hosting view's observation of this window-owned presentation.
    popover.contentViewController = nil
  }
}

private struct SlopShareView: View {
  @ObservedObject var model: SlopSharePopover
  let filename: String

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      VStack(alignment: .leading, spacing: 4) {
        Text("Share & Save").font(.title3.bold())
        Text(filename).font(.subheadline).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
      }
      VStack(spacing: 6) {
        if model.canExport {
          Grid(horizontalSpacing: 6, verticalSpacing: 6) {
            GridRow {
              formatOption(
                "Share PDF", hint: "Send a PDF using the sharing picker.", symbol: "doc.richtext", command: .sharePDF)
              formatOption(
                "Save PDF", hint: "Choose where to save a PDF.", symbol: "arrow.down.to.line", command: .exportPDF)
            }
            GridRow {
              formatOption(
                "Share Image", hint: "Send a PNG image using the sharing picker.", symbol: "photo", command: .sharePNG)
              formatOption(
                "Save Image", hint: "Choose where to save a PNG image.", symbol: "arrow.down.to.line",
                command: .exportPNG
              )
            }
          }
        }
        option("Share Slop File", detail: "An editable copy for hitSlop.", symbol: "doc", command: .shareSlop)
        if model.sharing == nil {
          option("Share URL", detail: "Coming soon", symbol: "link", command: nil)
        }
      }.disabled(!model.commandsEnabled || model.preparing != nil)
      if let preparing = model.preparing {
        HStack(spacing: 8) {
          ProgressView().controlSize(.small)
          Text(preparing).font(.callout).foregroundStyle(.secondary)
        }.accessibilityElement(children: .combine)
      }
      Divider()
      if let sharing = model.sharing {
        LiveCollaborationSection(
          model: sharing, interactionChanged: model.setCollaborationInteracting,
          saveUnsharedCopy: { model.choose(.duplicate) }
        ).disabled(!model.commandsEnabled || model.preparing != nil)
      } else {
        Toggle(isOn: .constant(false)) {
          VStack(alignment: .leading, spacing: 5) {
            Text("Live collaboration").font(.subheadline.weight(.medium))
            Text("Coming soon").font(.caption)
              .padding(.horizontal, 7).padding(.vertical, 3)
              .background(.quaternary, in: Capsule())
          }
        }.toggleStyle(.switch).controlSize(.small).disabled(true)
          .accessibilityLabel("Live collaboration, coming soon")
      }
    }
    .padding(18).frame(width: 320)
    .onExitCommand { if model.canDismiss { model.dismiss() } }
  }

  private func formatOption(_ title: String, hint: String, symbol: String, command: SlopDocumentCommand) -> some View {
    Button {
      model.choose(command)
    } label: {
      VStack(spacing: 8) {
        Image(systemName: symbol).font(.system(size: 19)).foregroundStyle(.secondary)
          .frame(height: 24)
        Text(title).font(.body.weight(.medium))
      }.padding(.vertical, 12).frame(maxWidth: .infinity).contentShape(Rectangle())
    }.buttonStyle(.plain)
      .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
      .accessibilityLabel(title).accessibilityHint(hint)
  }

  private func option(_ title: String, detail: String, symbol: String, command: SlopDocumentCommand?) -> some View {
    Button {
      if let command { model.choose(command) }
    } label: {
      HStack(spacing: 12) {
        Image(systemName: symbol).font(.system(size: 19)).foregroundStyle(.secondary).frame(width: 26)
        VStack(alignment: .leading, spacing: 3) {
          Text(title).font(.body.weight(.medium))
          Text(detail).font(.caption).foregroundStyle(.secondary)
        }
        Spacer(minLength: 0)
        Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
      }.padding(10).frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
    }.buttonStyle(.plain)
      .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
      .accessibilityLabel(title).accessibilityHint(detail)
      .disabled(command == nil)
  }
}
