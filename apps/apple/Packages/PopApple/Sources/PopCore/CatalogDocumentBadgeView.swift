import SwiftUI

/// One quiet exception or sharing badge; the containing tile supplies its full accessible meaning.
public struct CatalogDocumentBadgeView: View {
  public let badge: CatalogDocumentBadge
  public init(badge: CatalogDocumentBadge) { self.badge = badge }
  public var body: some View {
    Image(systemName: symbol)
      .font(.system(size: 12, weight: .semibold))
      .foregroundStyle(badge == .attention ? Color.orange : PopBrand.ink.opacity(0.7))
      .frame(width: 26, height: 26)
      .background(PopBrand.cream, in: Circle())
      .overlay(Circle().strokeBorder(PopBrand.ink.opacity(0.06)))
      .shadow(color: .black.opacity(0.10), radius: 3, y: 2)
      .accessibilityHidden(true)
  }
  private var symbol: String {
    switch badge {
    case .attention: "exclamationmark.triangle.fill"
    case .undownloaded: "icloud.and.arrow.down"
    case .shared: "person.2.fill"
    }
  }
}
