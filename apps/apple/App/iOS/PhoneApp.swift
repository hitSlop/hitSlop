import SwiftUI
import UIKit

final class PhoneAppDelegate: NSObject, UIApplicationDelegate {
  func application(
    _ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    application.registerForRemoteNotifications()
    return true
  }
}

@main struct HitSlopPhoneApp: App {
  @UIApplicationDelegateAdaptor(PhoneAppDelegate.self) var delegate

  var body: some Scene {
    WindowGroup { PhoneRootView() }
  }
}
