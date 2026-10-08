import Foundation

/// Every deadline the document layer keeps.
public enum Timeouts {
  /// A socket command, from admission to its reply (`NativeCommandDeadline`).
  public static let command: Duration = .seconds(30)
  /// A page's mount, and its app's unmount as its window closes.
  public static let pageReady: Duration = .seconds(15)
  static let unmount: Duration = .seconds(2)
  /// How long an open runs before its progress panel appears.
  public static let progressDelay: Duration = .seconds(1)
}
