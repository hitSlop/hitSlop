import AppKit
import PopCore
import SwiftUI

enum CatalogStyle {
  // Colors come from `PopBrand`; these are the catalog's derived roles.
  /// Soft tint behind the selected row.
  static let selection = PopBrand.accent.opacity(0.07)
  /// Thin accent outline for the selected row.
  static let selectionStroke = PopBrand.accent.opacity(0.16)
  static let hover = PopBrand.ink.opacity(0.05)
  static let hairline = PopBrand.ink.opacity(0.09)
  static let fill = PopBrand.ink.opacity(0.05)
  static let sidebar = PopBrand.creamDeep
  static let content = PopBrand.cream
  static let card = PopBrand.card
  /// Height of the titlebar strip the traffic lights sit in.
  static let titlebar: CGFloat = 52
}

struct KeyCap: View {
  let label: String
  var foreground: Color = .secondary
  var border: Color = CatalogStyle.hairline
  init(_ label: String, foreground: Color = .secondary, border: Color = CatalogStyle.hairline) {
    self.label = label
    self.foreground = foreground
    self.border = border
  }
  var body: some View {
    Text(label)
      .font(.caption2.weight(.medium))
      .foregroundStyle(foreground)
      .padding(.horizontal, 5).padding(.vertical, 1.5)
      .overlay(RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(border))
  }
}

/// Last component of an abbreviated folder path, e.g. "~/Desktop" → "Desktop".
func folderName(_ folderPath: String) -> String {
  let name = (folderPath as NSString).lastPathComponent
  return name.isEmpty ? folderPath : name
}
