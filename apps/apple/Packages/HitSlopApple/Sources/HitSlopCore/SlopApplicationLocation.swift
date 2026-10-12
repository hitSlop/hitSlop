import Foundation
import MachO

/// Select the application used by a native helper's open request.
public enum SlopApplicationLocation {
  public static func application(registered: () -> URL?) -> URL? {
    // The launcher controls argv[0]; dyld knows which executable is actually running.
    var size: UInt32 = 0
    _NSGetExecutablePath(nil, &size)
    let path = UnsafeMutablePointer<CChar>.allocate(capacity: Int(size))
    defer { path.deallocate() }
    guard _NSGetExecutablePath(path, &size) == 0 else { return registered() }
    return application(for: URL(fileURLWithPath: String(cString: path)), registered: registered)
  }

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
