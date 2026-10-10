import Foundation

/// Every deadline the document layer keeps.
public enum Timeouts {
  /// A socket command, from admission to its reply (`NativeCommandDeadline`).
  public static let command: Duration = .seconds(30)
  /// A page's mount, including the app's initial rendering.
  public static let pageReady: Duration = .seconds(15)
  /// Grace for app teardown after the owner has saved and released the document.
  static let unmount: Duration = .seconds(2)
  /// How long an open runs before its progress panel appears.
  public static let progressDelay: Duration = .seconds(1)
}
