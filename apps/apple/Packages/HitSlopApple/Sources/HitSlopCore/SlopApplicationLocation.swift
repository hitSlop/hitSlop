import Foundation

/// Select the application used by a native helper's open request.
public enum SlopApplicationLocation {
  public static func application(for helper: URL, registered: () -> URL?) -> URL? {
    let helpers = helper.resolvingSymlinksInPath().deletingLastPathComponent()
    let contents = helpers.deletingLastPathComponent()
    let app = contents.deletingLastPathComponent()
    if helpers.lastPathComponent == "Helpers", contents.lastPathComponent == "Contents",
      app.pathExtension == "app", Bundle(url: app)?.bundleIdentifier == "com.hitslop.app"
    {
      return app
    }
    return registered()
  }
}
