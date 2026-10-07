import FirebaseAnalytics
import FirebaseCore
import FirebaseCrashlytics
import Foundation
import HitSlopCore

public enum HitSlopFirebase {
  public static func configure() {
    // Debug and test hosts never initialize Firebase, including its startup collection.
    #if !DEBUG
      guard FirebaseApp.allApps?.isEmpty != false else { return }
      FirebaseApp.configure()
      Crashlytics.crashlytics().setCrashlyticsCollectionEnabled(true)
      Analytics.setAnalyticsCollectionEnabled(true)
    #endif
  }

  @MainActor private static let policy = SlopTelemetryPolicy()
  @MainActor public static let telemetry = SlopTelemetry { event in
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
            domain: "com.hitslop.operation.\(label)", code: context.code(for: operation),
            userInfo: [NSLocalizedDescriptionKey: label]), userInfo: context.fields(for: operation))
      }
    #endif
  }
}
