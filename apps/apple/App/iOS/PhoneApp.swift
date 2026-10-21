import BackgroundTasks
import CloudKit
import HitSlopCore
import SwiftUI
import UIKit

@MainActor final class PhoneAppDelegate: NSObject, UIApplicationDelegate {
  static let refreshIdentifier = "com.hitslop.app.widgets.refresh"
  // Constructed during launch, including a silent-push launch without a scene.
  let model = PhoneDocuments()

  func application(
    _ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.refreshIdentifier, using: .main) { [weak self] task in
      MainActor.assumeIsolated {
        guard let self, let refresh = task as? BGAppRefreshTask else {
          task.setTaskCompleted(success: false)
          return
        }
        Self.scheduleRefresh()
        let completion = PhoneRefreshCompletion { success in
          refresh.expirationHandler = nil
          refresh.setTaskCompleted(success: success)
        }
        let work = Task { @MainActor in
          let result = await self.refreshWidgets()
          completion.finish(result != .failed)
        }
        refresh.expirationHandler = {
          work.cancel()
          Task { @MainActor in completion.finish(false) }
        }
      }
    }
    SlopLibrary.shared.activated(application.applicationState == .active)
    application.registerForRemoteNotifications()
    return true
  }

  static func scheduleRefresh() {
    let request = BGAppRefreshTaskRequest(identifier: Self.refreshIdentifier)
    request.earliestBeginDate = Date(timeIntervalSinceNow: 30 * 60)
    do { try BGTaskScheduler.shared.submit(request) } catch {
      NSLog("hitSlop background refresh scheduling failed: %@", error.localizedDescription)
    }
  }

  func application(
    _ application: UIApplication, didReceiveRemoteNotification userInfo: [AnyHashable: Any],
    fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void
  ) {
    guard let notification = CKNotification(fromRemoteNotificationDictionary: userInfo),
      notification.containerIdentifier == "iCloud.com.hitslop.app"
    else {
      completionHandler(.noData)
      return
    }
    Task { completionHandler(await refreshWidgets()) }
  }

  private func refreshWidgets() async -> UIBackgroundFetchResult {
    let lease = PhoneRefreshLease()
    // UIKit calls this on the main thread; end the task before returning or the app is killed.
    lease.identifier = UIApplication.shared.beginBackgroundTask(withName: "Refresh library widgets") {
      MainActor.assumeIsolated { lease.expire() }
    }
    guard lease.identifier != .invalid else { return .failed }
    defer { lease.end() }
    let operation = Task { @MainActor [self] in
      model.widgets.beginBackgroundRefresh()
      defer { model.widgets.endBackgroundRefresh() }
      let before = SlopLibrary.shared.entries
      do {
        try Task.checkCancellation()
        try await SlopLibrary.shared.fetchInBackground()
        try Task.checkCancellation()
        await model.widgets.refreshPlaced()
        try Task.checkCancellation()
        return SlopLibrary.shared.entries == before ? UIBackgroundFetchResult.noData : .newData
      } catch {
        if !Task.isCancelled { NSLog("hitSlop background refresh failed: %@", error.localizedDescription) }
        return .failed
      }
    }
    lease.attach(operation)
    return await withTaskCancellationHandler {
      await operation.value
    } onCancel: {
      operation.cancel()
    }
  }
}

/// Expiration and normal completion can race; each system completion runs once.
@MainActor private final class PhoneRefreshCompletion {
  private var completion: ((Bool) -> Void)?
  init(_ completion: @escaping (Bool) -> Void) { self.completion = completion }
  func finish(_ success: Bool) {
    let callback = completion
    completion = nil
    callback?(success)
  }
}

@MainActor private final class PhoneRefreshLease {
  var identifier: UIBackgroundTaskIdentifier = .invalid
  private var work: Task<UIBackgroundFetchResult, Never>?
  private var expired = false
  /// Work attached after iOS already expired the task is cancelled at once.
  func attach(_ operation: Task<UIBackgroundFetchResult, Never>) {
    work = operation
    if expired { operation.cancel() }
  }
  func expire() {
    expired = true
    work?.cancel()
    end()
  }
  func end() {
    guard identifier != .invalid else { return }
    UIApplication.shared.endBackgroundTask(identifier)
    identifier = .invalid
  }
}

@main struct HitSlopPhoneApp: App {
  @UIApplicationDelegateAdaptor(PhoneAppDelegate.self) var delegate

  var body: some Scene {
    WindowGroup { PhoneRootView(model: delegate.model) }
  }
}
