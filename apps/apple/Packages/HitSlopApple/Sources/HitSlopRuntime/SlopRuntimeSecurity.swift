import Foundation
import HitSlopDocument

public enum SlopRuntimePurpose: Sendable {
  case interactive, backgroundRender

  /// Background renders read the saved document without owning or modifying the package.
  var storageMode: StorageMode { self == .interactive ? .document : .snapshot }
}
