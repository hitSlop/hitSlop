import Foundation

/// Where hitSlop sends people: the guide, the source and the community. The catalog sidebar and
/// the failure screen open these same addresses.
public enum SlopLinks {
  public static let authoring = URL(string: "https://hitslop.com/docs/getting-started/")!
  public static let github = URL(string: "https://github.com/hitslop/hitslop")!
  public static let discord = URL(string: "https://discord.gg/cqKRZjAWv3")!
  // TODO: replace with the public TestFlight invitation before publishing.
  public static let iosDownload = URL(string: "https://testflight.apple.com/join/REPLACE_ME")!
}
