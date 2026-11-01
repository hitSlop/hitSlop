import AppKit
import SwiftUI

enum CatalogStyle {
  static let brand = Color(red: 0.45, green: 0.16, blue: 0.96)
  static let brandHover = Color(red: 0.53, green: 0.24, blue: 0.98)
  static let selection = brand.opacity(0.10)
  static let hover = Color.primary.opacity(0.04)
  static let hairline = Color.primary.opacity(0.07)
  static let fill = Color.primary.opacity(0.045)
  static let sidebar = Color(nsColor: .windowBackgroundColor)
  static let content = Color(nsColor: .textBackgroundColor)
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
