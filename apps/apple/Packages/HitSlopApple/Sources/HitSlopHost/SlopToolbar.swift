import AppKit
import HitSlopCore
import SwiftUI

func slopToolbarFrame(document: NSRect, visible: NSRect?) -> NSRect {
  let width = min(max(document.width, 360), 560, max(0, (visible?.width ?? 576) - 16))
  var frame = NSRect(x: document.midX - width / 2, y: document.maxY + 8, width: width, height: 44)
  if let visible {
    frame.origin.x = min(max(frame.minX, visible.minX + 8), visible.maxX - width - 8)
    if frame.maxY > visible.maxY { frame.origin.y = document.maxY - frame.height - 10 }
    frame.origin.y = min(max(frame.minY, visible.minY + 8), visible.maxY - frame.height - 10)
  }
  return frame
}

@MainActor final class SlopToolbarDragHandleView: NSImageView {
  var onDrag: (NSEvent) -> Void

  init(onDrag: @escaping (NSEvent) -> Void) {
    self.onDrag = onDrag
    super.init(frame: .zero)
    image = NSImage(
      systemSymbolName: "circle.grid.3x3.fill", accessibilityDescription: "Drag window")?
      .withSymbolConfiguration(.init(pointSize: 8, weight: .medium))
    imageScaling = .scaleProportionallyDown
    contentTintColor = .secondaryLabelColor
    setAccessibilityElement(true)
    setAccessibilityLabel("Drag window")
  }

  required init?(coder: NSCoder) { nil }
  override func resetCursorRects() {
    super.resetCursorRects()
    addCursorRect(bounds, cursor: .openHand)
  }
  override func mouseDown(with event: NSEvent) { onDrag(event) }
}

private struct ToolbarDragHandle: NSViewRepresentable {
  let onDrag: (NSEvent) -> Void

  func makeNSView(context: Context) -> SlopToolbarDragHandleView {
    SlopToolbarDragHandleView(onDrag: onDrag)
  }
  func updateNSView(_ view: SlopToolbarDragHandleView, context: Context) { view.onDrag = onDrag }
}

/// SwiftUI's macOS Menu rewrites custom labels as centered, tail-truncated titles.
/// A native button keeps the document identity flexible while using a standard NSMenu.
private struct ToolbarFileMenu: NSViewRepresentable {
  let identity: SlopDocumentIdentity
  let reveal: () -> Void
  let copyPath: () -> Void

  func makeNSView(context: Context) -> SlopToolbarFileButton { SlopToolbarFileButton() }
  func updateNSView(_ button: SlopToolbarFileButton, context: Context) {
    button.identity = identity
    button.reveal = reveal
    button.copyPath = copyPath
    button.isEnabled = context.environment.isEnabled
  }
}

@MainActor final class SlopToolbarFileButton: NSButton {
  private let filename = NSTextField(labelWithString: "")
  private let documentIcon = NSImageView()
  private let disclosure = NSImageView()
  var identity: SlopDocumentIdentity? {
    didSet {
      filename.stringValue = identity?.filename ?? ""
      toolTip = identity?.path
      setAccessibilityLabel("\(identity?.filename ?? "Document"), file actions")
    }
  }
  var reveal: () -> Void = {}
  var copyPath: () -> Void = {}

  override var isEnabled: Bool {
    didSet {
      filename.textColor = isEnabled ? .labelColor : .disabledControlTextColor
      documentIcon.contentTintColor = isEnabled ? .secondaryLabelColor : .disabledControlTextColor
      disclosure.contentTintColor = documentIcon.contentTintColor
    }
  }

  init() {
    super.init(frame: .zero)
    title = ""
    isBordered = false
    setButtonType(.momentaryPushIn)
    target = self
    action = #selector(showFileMenu)
    filename.font = .systemFont(ofSize: 13, weight: .semibold)
    filename.lineBreakMode = .byTruncatingMiddle
    filename.maximumNumberOfLines = 1
    filename.cell?.usesSingleLineMode = true
    filename.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    filename.setContentHuggingPriority(.defaultLow, for: .horizontal)
    documentIcon.image = NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
    disclosure.image = NSImage(systemSymbolName: "chevron.down", accessibilityDescription: nil)?
      .withSymbolConfiguration(.init(pointSize: 9, weight: .semibold))
    documentIcon.contentTintColor = .secondaryLabelColor
    disclosure.contentTintColor = .secondaryLabelColor
    for view in [documentIcon, filename, disclosure] {
      view.translatesAutoresizingMaskIntoConstraints = false
      view.setAccessibilityElement(false)
      addSubview(view)
      view.centerYAnchor.constraint(equalTo: centerYAnchor).isActive = true
    }
    NSLayoutConstraint.activate([
      documentIcon.leadingAnchor.constraint(equalTo: leadingAnchor),
      documentIcon.widthAnchor.constraint(equalToConstant: 14),
      documentIcon.heightAnchor.constraint(equalToConstant: 16),
      filename.leadingAnchor.constraint(equalTo: documentIcon.trailingAnchor, constant: 7),
      filename.trailingAnchor.constraint(equalTo: disclosure.leadingAnchor, constant: -7),
      disclosure.trailingAnchor.constraint(equalTo: trailingAnchor),
      disclosure.widthAnchor.constraint(equalToConstant: 10),
      disclosure.heightAnchor.constraint(equalToConstant: 12),
    ])
  }
  required init?(coder: NSCoder) { nil }
  override var intrinsicContentSize: NSSize { NSSize(width: NSView.noIntrinsicMetric, height: 28) }
  override func hitTest(_ point: NSPoint) -> NSView? { super.hitTest(point) == nil ? nil : self }

  @objc private func showFileMenu() {
    guard let identity else { return }
    let menu = NSMenu()
    menu.addItem(withTitle: identity.filename, action: nil, keyEquivalent: "")
    menu.addItem(withTitle: identity.folderPath, action: nil, keyEquivalent: "")
    menu.addItem(.separator())
    let revealItem = menu.addItem(withTitle: "Reveal in Finder", action: #selector(revealFile), keyEquivalent: "")
    revealItem.target = self
    let copyItem = menu.addItem(withTitle: "Copy Path", action: #selector(copyFilePath), keyEquivalent: "")
    copyItem.target = self
    menu.popUp(positioning: nil, at: NSPoint(x: 0, y: isFlipped ? bounds.maxY + 4 : bounds.minY - 4), in: self)
  }
  @objc private func revealFile() { reveal() }
  @objc private func copyFilePath() { copyPath() }
}

struct SlopToolbar: View {
  /// What a toolbar control asks its window to do.
  enum Action {
    /// A document operation, which the app runs like the menu bar's.
    case document(SlopDocumentCommand)
    case close, minimize, togglePin, toggleTheme, reveal, copyPath
    case openEditor(URL)
  }
  /// What the controls show and whether each is available.
  struct Controls {
    var pinned = false, canPin = false
    /// Whether the theme panel is shown, and whether showing or hiding it is available.
    var themeShown = false, canToggleTheme = false
    /// Whether the app accepts document operations now.
    var commandsEnabled = false
  }
  let identity: SlopDocumentIdentity
  let controls: Controls
  let menuTrackingChanged: (Bool) -> Void
  let drag: (NSEvent) -> Void
  let act: (Action) -> Void
  let editors: [(String, URL)]
  var body: some View {
    HStack(spacing: 6) {
      ToolbarDragHandle(onDrag: drag).frame(width: 18, height: 28).help("Drag window")
      HStack(spacing: 0) {
        windowControl("xmark", "Close", .red) { act(.close) }
        windowControl("minus", "Minimize", .yellow) { act(.minimize) }
      }.fixedSize().background(SlopToolbarControlRegion())
      Divider().frame(height: 20).padding(.horizontal, 2)
      ToolbarFileMenu(identity: identity, reveal: { act(.reveal) }, copyPath: { act(.copyPath) })
        .frame(minWidth: 0, maxWidth: .infinity).frame(height: 28)
      icon(controls.pinned ? "pin.fill" : "pin", controls.pinned ? "Unpin" : "Always on Top") { act(.togglePin) }
        .disabled(!controls.canPin).background(SlopToolbarControlRegion())
      icon(controls.themeShown ? "paintpalette.fill" : "paintpalette", controls.themeShown ? "Hide Theme" : "Theme") {
        act(.toggleTheme)
      }.disabled(!controls.canToggleTheme).background(SlopToolbarControlRegion())
      icon("square.and.arrow.up", "Share a Copy…") {
        act(.document(.share))
      }.disabled(!controls.commandsEnabled).background(SlopToolbarControlRegion())
      Menu {
        Button("Duplicate…") { act(.document(.duplicate)) }.disabled(!controls.commandsEnabled)
        Divider()
        Button("Export PNG…") { act(.document(.exportPNG)) }.disabled(!controls.commandsEnabled)
        Button("Export PDF…") { act(.document(.exportPDF)) }.disabled(!controls.commandsEnabled)
        if !editors.isEmpty {
          Divider()
          ForEach(editors, id: \.1) { editor in Button(editor.0) { act(.openEditor(editor.1)) } }
        }
      } label: {
        Image(systemName: "ellipsis").frame(width: 28, height: 28).contentShape(Rectangle())
      }
      .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
      .background(SlopToolbarControlRegion())
      .help("More actions").accessibilityLabel("More document actions")
    }
    .padding(.horizontal, 10).frame(height: 40)
    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 13))
    .overlay(RoundedRectangle(cornerRadius: 13).stroke(.primary.opacity(0.12)))
    .padding(2)
    .onReceive(NotificationCenter.default.publisher(for: NSMenu.didBeginTrackingNotification)) { _ in
      menuTrackingChanged(true)
    }
    .onReceive(NotificationCenter.default.publisher(for: NSMenu.didEndTrackingNotification)) { _ in
      menuTrackingChanged(false)
    }
  }
  private func windowControl(_ symbol: String, _ label: String, _ color: Color, _ action: @escaping () -> Void)
    -> some View
  {
    Button(action: action) {
      Image(systemName: symbol).font(.system(size: 8, weight: .bold))
        .foregroundStyle(.black.opacity(0.65))
        .frame(width: 13, height: 13)
        .background(color, in: Circle())
        .overlay(Circle().stroke(.black.opacity(0.12)))
        .frame(width: 22, height: 28).contentShape(Rectangle())
    }.buttonStyle(.plain).help(label).accessibilityLabel(label)
  }
  private func icon(_ name: String, _ help: String, _ action: @escaping () -> Void) -> some View {
    Button(action: action) {
      Image(systemName: name).frame(width: 28, height: 28).contentShape(Rectangle())
    }.buttonStyle(.plain).help(help).accessibilityLabel(help)
  }
}
