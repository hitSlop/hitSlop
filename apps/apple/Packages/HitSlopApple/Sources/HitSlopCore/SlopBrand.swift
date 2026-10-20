import CoreText
import SwiftUI

/// Shared catalog branding. Authored documents retain their own fonts and artwork.
public enum SlopBrand {
  private static let registered: Bool = {
    guard let url = Bundle.module.url(forResource: "Baloo2", withExtension: "ttf") else { return false }
    var error: Unmanaged<CFError>?
    if CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error) { return true }
    return error.map { CFErrorGetCode($0.takeRetainedValue()) == CTFontManagerError.alreadyRegistered.rawValue }
      ?? false
  }()

  public static func heading(_ size: CGFloat, relativeTo style: Font.TextStyle = .headline) -> Font {
    registered
      ? .custom("Baloo2-Regular", size: size, relativeTo: style).weight(.bold) : .system(style, design: .rounded).bold()
  }
}

public struct SlopBrandMark: View {
  private let size: CGFloat
  public init(size: CGFloat = 34) { self.size = size }
  public var body: some View {
    Image("BrandIcon", bundle: .module).resizable().scaledToFit()
      .frame(width: size, height: size).accessibilityHidden(true)
  }
}
