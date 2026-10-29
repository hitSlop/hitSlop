import TinysCoreBinding

private let host = hostLimits()
public enum Limits {
  public static let errorText = Int(host.errorText)
  public static let storageBytes = Int(host.storageBytes)
  public static let themeFile = Int(host.themeFile)
  public static let pushItems = Int(host.pushItems)
  public static let pushBytes = Int(host.pushBytes)
}
public enum WindowBounds {
  public static let minWidth = Int(host.minWidth)
  public static let minHeight = Int(host.minHeight)
}
public enum PackageFormat { public static let level = Int(host.packageFormat) }
public enum RuntimeABI { public static let level = Int(host.runtimeAbi) }
public enum EngineProtocol { public static let version = Int(host.engineProtocol) }
public enum AppResourcePolicy { public static let contentSecurityPolicy = appResourcePolicy() }
