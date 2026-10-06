import Foundation
import HitSlopCoreBinding

/// A document's palette as the window shows it: colors by token, and the owner's theme
/// revision it was read at. Revisions only grow, so a later palette has a higher one.
public struct SlopThemeState: Sendable, Equatable {
  public let overrides: [String: String]
  public let effective: [String: String]
  public let revision: Int
  public init(overrides: [String: String], effective: [String: String], revision: Int) {
    self.overrides = overrides
    self.effective = effective
    self.revision = revision
  }
  init(_ read: DocumentOwner.ThemeRead) throws {
    func colors(_ json: String) throws -> [String: String] {
      try JSONDecoder().decode([String: String].self, from: Data(json.utf8))
    }
    self.init(
      overrides: try colors(read.state.overrides), effective: try colors(read.state.effective), revision: read.revision)
  }
}

/// A change made in the window's theme panel.
public enum SlopThemeChange: Sendable, Equatable {
  /// Sets colors; a color equal to the template's resets that token.
  case set([String: String])
  /// Returns every color to the template's.
  case resetAll
  /// Replaces the palette with a theme file's.
  case importFile(String)
}
