import PopCoreBinding

/// A saved sharing relationship, independent of connection status or presence.
public enum CatalogSharingRelationship: Equatable, Sendable {
  case none
  case sharedByYou
  case sharedWithYou

  fileprivate var accessibilityDescription: String? {
    switch self {
    case .none: nil
    case .sharedByYou: "Sharing enabled"
    case .sharedWithYou: "Shared with you"
    }
  }
}

public enum CatalogDocumentBadge: Equatable, Sendable {
  case attention
  case undownloaded
  case shared
}

/// The same catalog meaning on both hosts. Rendering and discovery stay with the host;
/// projecting a saved entry never starts a connection or reads a file.
public struct CatalogDocumentPresentation: Equatable, Sendable {
  public let badge: CatalogDocumentBadge?
  public let accessibilityValue: String
  private let downloaded: Bool
  private let localOnly: Bool
  private let sharing: CatalogSharingRelationship

  public init(
    needsAttention: Bool, downloaded: Bool, localOnly: Bool,
    sharing: CatalogSharingRelationship
  ) {
    self.downloaded = downloaded
    self.localOnly = localOnly
    self.sharing = sharing
    let primary: String
    if needsAttention {
      badge = .attention
      primary = "Needs attention"
    } else if !downloaded {
      badge = .undownloaded
      primary = "Not downloaded"
    } else {
      badge = sharing == .none ? nil : .shared
      primary = localOnly ? "Saved on this device" : "Sync across devices"
    }
    accessibilityValue = [primary, sharing.accessibilityDescription].compactMap { $0 }.joined(separator: ", ")
  }

  /// Cached host failures use the same priority and sharing accessibility as saved entries.
  public func requiringAttention() -> CatalogDocumentPresentation {
    CatalogDocumentPresentation(needsAttention: true, downloaded: downloaded, localOnly: localOnly, sharing: sharing)
  }

  public init(entry: LibraryEntry, needsAttention: Bool = false) {
    let sharing: CatalogSharingRelationship
    if entry.role == .editor {
      sharing = .sharedWithYou
    } else if entry.role == .owner && entry.sharingEnabled == true {
      sharing = .sharedByYou
    } else {
      sharing = .none
    }
    self.init(
      needsAttention: needsAttention || entry.problem != nil, downloaded: entry.downloaded,
      localOnly: entry.localOnly, sharing: sharing)
  }
}

extension LibraryEntry {
  public var catalogPresentation: CatalogDocumentPresentation { CatalogDocumentPresentation(entry: self) }
}
