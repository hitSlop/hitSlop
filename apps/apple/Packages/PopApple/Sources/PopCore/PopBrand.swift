import CoreText
import SwiftUI

#if canImport(AppKit)
  import AppKit
#elseif canImport(UIKit)
  import UIKit
#endif

/// Shared catalog branding. Authored documents retain their own fonts and artwork.
public enum PopBrand {
  private static let registered: Bool = {
    guard let url = Bundle.module.url(forResource: "Baloo2", withExtension: "ttf") else { return false }
    var error: Unmanaged<CFError>?
    if CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error) { return true }
    return error.map { CFErrorGetCode($0.takeRetainedValue()) == CTFontManagerError.alreadyRegistered.rawValue }
      ?? false
  }()

  // MARK: Palette
  //
  // The one owner of native catalog colors. Warm cream surfaces, near-black ink, and a single
  // restrained accent used only for selection tint, small badges and the primary action.
  // Dark mode stays neutral and keeps the accent unchanged.

  /// Magenta accent: selected-row tint, small badges, the primary action. Never a large wash.
  public static let accent = Color(red: 0.898, green: 0.098, blue: 0.498)  // #E5197F
  public static let accentHover = Color(red: 0.812, green: 0.071, blue: 0.451)  // #CF1273
  /// Near-black ink for primary pills and strong text; inverts in dark mode.
  public static let ink = dynamic(light: 0x14122B, dark: 0xF4F1EA)
  /// Text and glyphs placed on `ink`.
  public static let onInk = dynamic(light: 0xFFFFFF, dark: 0x14122B)
  /// Window content surface (cream).
  public static let cream = dynamic(light: 0xFBF7F0, dark: 0x1D1C1F)
  /// Sidebar surface, a touch deeper than `cream`.
  public static let creamDeep = dynamic(light: 0xF4EEE3, dark: 0x252428)
  /// Raised cards, icon tiles and fields.
  public static let card = dynamic(light: 0xFFFFFF, dark: 0x2B2A2F)
  /// Friendly folder and promo hues for decoration.
  public static let sky = Color(red: 0.30, green: 0.60, blue: 0.98)
  public static let lilac = Color(red: 0.62, green: 0.48, blue: 0.95)
  public static let sun = Color(red: 0.99, green: 0.76, blue: 0.22)
  public static let mint = Color(red: 0.30, green: 0.78, blue: 0.56)
  /// Soft yellow notice wash (warnings and update banners).
  public static let notice = dynamic(light: 0xFDF1C9, dark: 0x3A331C)

  private static func dynamic(light: UInt32, dark: UInt32) -> Color {
    func rgb(_ hex: UInt32) -> (CGFloat, CGFloat, CGFloat) {
      (CGFloat((hex >> 16) & 0xFF) / 255, CGFloat((hex >> 8) & 0xFF) / 255, CGFloat(hex & 0xFF) / 255)
    }
    let (l, d) = (rgb(light), rgb(dark))
    #if canImport(AppKit)
      return Color(
        nsColor: NSColor(name: nil) { appearance in
          let isDark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
          let c = isDark ? d : l
          return NSColor(srgbRed: c.0, green: c.1, blue: c.2, alpha: 1)
        })
    #else
      return Color(
        uiColor: UIColor { traits in
          let c = traits.userInterfaceStyle == .dark ? d : l
          return UIColor(red: c.0, green: c.1, blue: c.2, alpha: 1)
        })
    #endif
  }

  public static func heading(_ size: CGFloat, relativeTo style: Font.TextStyle = .headline) -> Font {
    registered
      ? .custom("Baloo2-Regular", size: size, relativeTo: style).weight(.bold) : .system(style, design: .rounded).bold()
  }
}

public struct PopBrandMark: View {
  private let size: CGFloat
  public init(size: CGFloat = 34) { self.size = size }
  public var body: some View {
    Image("BrandCharacter", bundle: .module).resizable().scaledToFit()
      .frame(width: size, height: size).accessibilityHidden(true)
  }
}
