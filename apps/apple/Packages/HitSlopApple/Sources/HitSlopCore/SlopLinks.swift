import Foundation

/// Where hitSlop sends people: the guide, the source and the community. The catalog sidebar and
/// the failure screen open these same addresses.
public enum SlopLinks {
  public static let authoring = URL(string: "https://hitslop.com/docs/getting-started/")!
  public static let github = URL(string: "https://github.com/hitslop/hitslop")!
  public static let discord = URL(string: "https://discord.gg/cqKRZjAWv3")!
  /// Temporary until the public TestFlight link.
  public static let iosDownload = URL(string: "https://hitslop.com/docs/availability/#get-the-iphone-app")!
}
