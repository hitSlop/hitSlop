import HitSlopCoreBinding

private let host = hostLimits()
public enum Limits {
  public static let errorText = Int(host.errorText)
  public static let storageBytes = Int(host.storageBytes)
  public static let themeFile = Int(host.themeFile)
  public static let pushItems = Int(host.pushItems)
  public static let pushBytes = Int(host.pushBytes)
  public static let socketRequest = Int(host.socketRequest)
  public static let imageSide = Int(host.imageSide)
  public static let imagePixels = Int(host.imagePixels)
}
public enum WindowBounds {
  public static let minWidth = Int(host.minWidth)
  public static let minHeight = Int(host.minHeight)
}
public enum HelperProtocol { public static let version = Int(host.helperProtocol) }
public enum AppResourcePolicy { public static let contentSecurityPolicy = appResourcePolicy() }
