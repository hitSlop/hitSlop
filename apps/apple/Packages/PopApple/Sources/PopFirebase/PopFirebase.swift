import FirebaseAnalytics
import FirebaseCore
import FirebaseCrashlytics
import Foundation
import PopCore

public enum PopFirebase {
  /// Configures the one default Firebase app (Auth, Firestore, Crashlytics, Analytics) on macOS and iOS.
  /// Telemetry collection is on only outside Debug; the Info.plist flags keep it off until this runs.
  public static func configure() {
    guard FirebaseApp.app() == nil,
      Bundle.main.path(forResource: "GoogleService-Info", ofType: "plist") != nil
    else { return }
    FirebaseApp.configure()
    #if DEBUG
      let collecting = false
    #else
      let collecting = true
    #endif
    Crashlytics.crashlytics().setCrashlyticsCollectionEnabled(collecting)
    Analytics.setAnalyticsCollectionEnabled(collecting)
  }

  @MainActor private static let policy = PopTelemetryPolicy()
  @MainActor public static let telemetry = PopTelemetry { event in
    #if !DEBUG
      guard FirebaseApp.app() != nil else { return }
      switch event {
      case .launched: Analytics.logEvent("app_launched", parameters: nil)
      case .opened: Analytics.logEvent("document_opened", parameters: nil)
      case .duplicated: Analytics.logEvent("document_duplicated", parameters: nil)
      case .themeEditorOpened: Analytics.logEvent("theme_editor_opened", parameters: nil)
      case .themeImported: Analytics.logEvent("theme_imported", parameters: nil)
      case .themeExported: Analytics.logEvent("theme_exported", parameters: nil)
      case .created(let source):
        Analytics.logEvent("document_created", parameters: ["source": source.rawValue])
      case .exported(let format):
        Analytics.logEvent("document_exported", parameters: ["format": format.fileExtension])
      case .breadcrumb(let operation, let phase):
        Crashlytics.crashlytics().log("\(operation.rawValue).\(phase.rawValue)")
      case .failed(let operation, let context):
        guard policy.shouldRecord(operation, context: context) else { return }
        let label = "\(operation.rawValue).\(context.classification.rawValue).\(context.reason.rawValue)"
        Crashlytics.crashlytics().record(
          error: NSError(
            // Fixed domains also separate categories on backends that merge different codes.
            domain: "click.dotpop.operation.\(label)", code: context.code(for: operation),
            userInfo: [NSLocalizedDescriptionKey: label]), userInfo: context.fields(for: operation))
      }
    #endif
  }
}
