import CoreGraphics
import HitSlopDocument
import SwiftUI

/// Palette colors as the core spells them: lowercase `#rrggbb`, or `#rrggbbaa` below full
/// opacity, in sRGB.
enum SlopThemeColor {
  static let space = CGColorSpace(name: CGColorSpace.sRGB)!
  /// A color as typed (`#abc`, `ABC123`, `#aabbcc80`) in the palette's spelling, or nil.
  /// The core still decides what it accepts.
  static func normalized(_ text: String) -> String? {
    var hex = text.trimmingCharacters(in: .whitespaces).lowercased()
    if hex.hasPrefix("#") { hex.removeFirst() }
    guard hex.allSatisfy(\.isHexDigit) else { return nil }
    switch hex.count {
    case 3, 4: hex = String(hex.flatMap { [$0, $0] })
    case 6, 8: break
    default: return nil
    }
    if hex.count == 8, hex.hasSuffix("ff") { hex.removeLast(2) }
    return "#" + hex
  }
  static func color(_ value: String) -> CGColor? {
    guard let hex = normalized(value), let bits = UInt64(hex.dropFirst(), radix: 16) else { return nil }
    let rgba = hex.count == 9 ? bits : bits << 8 | 0xff
    let components = [24, 16, 8, 0].map { CGFloat((rgba >> UInt64($0)) & 0xff) / 255 }
    return CGColor(colorSpace: space, components: components)
  }
  static func hex(_ color: CGColor) -> String? {
    guard let srgb = color.converted(to: space, intent: .defaultIntent, options: nil),
      let components = srgb.components, components.count == 4
    else { return nil }
    let bytes = components.map { Int((min(max($0, 0), 1) * 255).rounded()) }
    let rgb = bytes.prefix(3).map { String(format: "%02x", $0) }.joined()
    return "#" + rgb + (bytes[3] < 255 ? String(format: "%02x", bytes[3]) : "")
  }
  /// `graphiteDeep` or `graphite-deep` as "Graphite Deep".
  static func label(_ token: String) -> String {
    var words: [String] = [], word = ""
    for character in token {
      if character == "-" || (character.isUppercase && !word.isEmpty) {
        if !word.isEmpty { words.append(word) }
        word = character == "-" ? "" : String(character)
      } else { word.append(character) }
    }
    if !word.isEmpty { words.append(word) }
    return words.map { $0.prefix(1).uppercased() + $0.dropFirst() }.joined(separator: " ")
  }
}

/// The panel's view state. The owner holds the palette; a color being changed shows here
/// until a palette the owner read after accepting it arrives. Drafts retire by the owner's
/// theme revision, never by matching colors, so a change that overtakes one still shows.
@MainActor @Observable final class SlopThemeEditorModel {
  struct Row: Identifiable {
    let id: String
    let label: String
    let original: String
  }
  private struct Draft {
    let id: Int
    let value: String
    /// The owner's theme revision once it accepted this change.
    var accepted: Int?
  }
  typealias Send = (SlopThemeChange, @escaping @MainActor (Result<Int, Error>) -> Void) -> Void
  let rows: [Row]
  private(set) var effective: [String: String] = [:]
  private var drafts: [String: Draft] = [:]
  private var revision = -1
  @ObservationIgnored private var nextDraft = 0
  private(set) var errors: [String: String] = [:]
  @ObservationIgnored private let send: Send

  init(tokens: [(name: String, value: String)], send: @escaping Send) {
    rows = tokens.map { Row(id: $0.name, label: SlopThemeColor.label($0.name), original: $0.value) }
    self.send = send
  }
  func value(_ row: Row) -> String { drafts[row.id]?.value ?? effective[row.id] ?? row.original }
  func isChanged(_ row: Row) -> Bool { value(row) != row.original }
  var hasChanges: Bool { rows.contains(where: isChanged) }

  func set(_ row: Row, _ color: String) {
    errors[row.id] = nil
    guard value(row) != color else { return }
    let draft = draft(row, color)
    send(.set([row.id: color])) { [weak self] result in
      switch result {
      case .success(let revision): self?.accept([row.id: draft], at: revision)
      case .failure(let error):
        if self?.drafts[row.id]?.id == draft { self?.drafts[row.id] = nil }
        self?.errors[row.id] = error.localizedDescription
      }
    }
  }
  /// A typed color; anything that is not one keeps the current color.
  func commit(_ row: Row, typed: String) {
    guard let color = SlopThemeColor.normalized(typed) else {
      errors[row.id] = "Use a color such as #335577"
      return
    }
    set(row, color)
  }
  /// The hex field lost focus or took Return. Only text the person typed is committed; an
  /// untouched field takes the latest color, so it never writes an old one back. Returns
  /// the text the field shows next.
  func finishTyping(_ row: Row, text: String, edited: Bool) -> String {
    if edited { commit(row, typed: text) }
    return value(row)
  }
  func reset(_ row: Row) { set(row, row.original) }
  func resetAll() {
    errors = [:]
    var pending: [String: Int] = [:]
    for row in rows where isChanged(row) { pending[row.id] = draft(row, row.original) }
    send(.resetAll) { [weak self] result in
      switch result {
      case .success(let revision): self?.accept(pending, at: revision)
      case .failure:
        for (token, id) in pending where self?.drafts[token]?.id == id { self?.drafts[token] = nil }
      }
    }
  }
  func apply(_ theme: SlopThemeState) {
    guard theme.revision >= revision else { return }
    revision = theme.revision
    effective = theme.effective
    drafts = drafts.filter { $0.value.accepted.map { $0 > theme.revision } ?? true }
  }

  private func draft(_ row: Row, _ color: String) -> Int {
    nextDraft += 1
    drafts[row.id] = Draft(id: nextDraft, value: color)
    return nextDraft
  }
  /// A later draft for the same color keeps waiting for its own acceptance.
  private func accept(_ accepted: [String: Int], at revision: Int) {
    for (token, id) in accepted where drafts[token]?.id == id {
      if revision <= self.revision { drafts[token] = nil } else { drafts[token]?.accepted = revision }
    }
  }
}

struct SlopThemeEditor: View {
  let model: SlopThemeEditorModel
  let title: String
  let close: () -> Void
  let importTheme: () -> Void
  let exportTheme: () -> Void

  var body: some View {
    VStack(spacing: 0) {
      HStack(alignment: .firstTextBaseline) {
        VStack(alignment: .leading, spacing: 1) {
          Text("Theme").font(.headline)
          Text(title).font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
        Spacer()
        Button(action: close) { Image(systemName: "xmark").frame(width: 22, height: 22).contentShape(Rectangle()) }
          .buttonStyle(.plain).help("Close").accessibilityLabel("Close theme")
      }
      .padding(.horizontal, 14).padding(.top, 12).padding(.bottom, 8)
      Divider()
      if model.rows.isEmpty {
        ContentUnavailableView("No Colors", systemImage: "paintpalette",
          description: Text("This template has no theme colors."))
      } else {
        ScrollView {
          LazyVStack(spacing: 0) {
            ForEach(model.rows) { row in SlopThemeRow(model: model, row: row) }
          }
          .padding(.vertical, 6)
        }
      }
      Divider()
      HStack {
        Button("Import…", action: importTheme)
        Button("Export…", action: exportTheme)
        Spacer()
        Button("Reset to Original", action: model.resetAll).disabled(!model.hasChanges)
          .help("Returns every color to the template's. Theme changes aren't undoable.")
      }
      .controlSize(.small)
      .padding(.horizontal, 14).padding(.vertical, 10)
    }
    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 13))
    .overlay(RoundedRectangle(cornerRadius: 13).stroke(.primary.opacity(0.12)))
  }
}

private struct SlopThemeRow: View {
  let model: SlopThemeEditorModel
  let row: SlopThemeEditorModel.Row
  @State private var text = ""
  /// Whether the person typed since the field last showed a color.
  @State private var edited = false
  @FocusState private var editing: Bool

  var body: some View {
    let value = model.value(row), changed = model.isChanged(row)
    VStack(alignment: .leading, spacing: 2) {
      HStack(spacing: 8) {
        ColorPicker(row.label, selection: Binding(
          get: { SlopThemeColor.color(value) ?? CGColor(gray: 0, alpha: 1) },
          set: { color in if let hex = SlopThemeColor.hex(color) { model.set(row, hex) } }),
          supportsOpacity: true)
          .labelsHidden()
        Circle().fill(Color.accentColor).frame(width: 5, height: 5).opacity(changed ? 1 : 0)
          .accessibilityHidden(true)
        // Middle truncation keeps the words that tell similar colors apart ("Done … Deep").
        Text(row.label).lineLimit(1).truncationMode(.middle).help(row.id).layoutPriority(1)
        Spacer(minLength: 4)
        TextField(row.label, text: Binding(get: { text }, set: { text = $0; edited = true }))
          .labelsHidden()
          .font(.system(.caption, design: .monospaced))
          .textFieldStyle(.roundedBorder)
          .frame(width: 80)
          .focused($editing)
          .onSubmit(finish)
        Button { model.reset(row) } label: {
          Image(systemName: "arrow.uturn.backward").frame(width: 18, height: 18).contentShape(Rectangle())
        }
        .buttonStyle(.plain).foregroundStyle(.secondary)
        .help("Use the template's color").accessibilityLabel("Reset \(row.label)")
        .opacity(changed ? 1 : 0).disabled(!changed)
      }
      if let error = model.errors[row.id] {
        Text(error).font(.caption2).foregroundStyle(.red).padding(.leading, 30)
      }
    }
    .controlSize(.small)
    .padding(.horizontal, 14).padding(.vertical, 3)
    .onAppear { text = value }
    .onChange(of: value) { _, next in if !edited { text = next } }
    .onChange(of: editing) { _, now in if !now { finish() } }
  }
  private func finish() {
    text = model.finishTyping(row, text: text, edited: edited)
    edited = false
  }
}
