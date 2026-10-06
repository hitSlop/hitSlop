import Foundation

/// Every deadline the document layer keeps. A client waits longer than the command deadline,
/// so a command that finishes in time always reaches it.
public enum Timeouts {
  /// A socket command, from admission to its reply (`NativeCommandDeadline`).
  public static let command: Duration = .seconds(30)
  /// A connection's first request line, then the rest of its exchange.
  static let requestRead: Duration = .seconds(10)
  static let connection: Duration = .seconds(30)
  /// A client's socket reads and writes.
  static let client: Duration = command + .seconds(5)
  /// How long a request waits for an owner that is opening or closing, and how often it
  /// looks again.
  static let admission: Duration = .seconds(2)
  static let admissionRetry: Duration = .milliseconds(50)
  /// A page's mount, and its app's unmount as its window closes.
  public static let pageReady: Duration = .seconds(15)
  static let unmount: Duration = .seconds(2)
  /// How long an open runs before its progress panel appears.
  public static let progressDelay: Duration = .seconds(1)
}

extension Duration {
  /// The same span for Dispatch.
  var dispatch: DispatchTimeInterval { .nanoseconds(Int(components.seconds * 1_000_000_000 + components.attoseconds / 1_000_000_000)) }
}
