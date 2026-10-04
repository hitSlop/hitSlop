import Foundation

public enum SlopPagePurpose: Sendable {
  case interactive, backgroundRender

  /// Background renders read the saved document without owning or modifying the file.
  var storageMode: StorageMode { self == .interactive ? .document : .snapshot }
}
