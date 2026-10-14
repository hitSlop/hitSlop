import Foundation
import HitSlopCore
import HitSlopDocument

/// What the failure screen says and what Copy details puts on the clipboard: plain words for the
/// person, a report for us. A value, so the words and the report are tested without a window
/// (`PageFailureNoticeTests`); the window controller builds it and `FailureOverlay` shows it.
struct PageFailureNotice: Equatable {
  /// The build and system that ran the slop.
  struct Host: Equatable {
    var app: String
    var core: String
    var system: String
    var architecture: String

    static var current: Self {
      let bundle = Bundle.main.infoDictionary
      let version = bundle?["CFBundleShortVersionString"] as? String ?? "unknown"
      let build = bundle?["CFBundleVersion"] as? String ?? "unknown"
      let system = ProcessInfo.processInfo.operatingSystemVersion
      #if arch(arm64)
        let architecture = "arm64"
      #else
        let architecture = "x86_64"
      #endif
      return Self(
        app: "\(version) (\(build))", core: DocumentOwner.coreBuildID,
        system: "macOS \(system.majorVersion).\(system.minorVersion).\(system.patchVersion)",
        architecture: architecture)
    }
  }

  let title: String
  let explanation: String
  /// The error and stack as the page reported them, for the collapsed technical details.
  let details: String
  /// Plain text for the clipboard: the build, the system, the slop and the failure, then the
  /// error and the top of its stack. It carries no document values.
  let report: String

  /// The report keeps the error and the top of its stack: enough to find the failure, short
  /// enough to paste into a chat message.
  static let excerptLines = 10
  static let excerptCharacters = 1_500

  init(
    message: String, reason: SlopFailureContext.Reason?, classification: SlopFailureContext.Classification = .platform,
    slopTitle: String, slopSlug: String, host: Host = .current
  ) {
    switch reason {
    case .authoredException:
      title = "This slop couldn't open"
      explanation =
        "Its own code failed while starting. If it opened before, a hitSlop update may have broken it, and we'd like to know."
    case .webContentTerminated:
      // The owner lives in the app, not in the view: it keeps every edit it accepted, and the
      // reopened page shows them (`rendererDeathKeepsCommittedEditsAndOwnership`). Only input
      // the page had not yet sent is lost. A slop whose own code failed to start may have
      // written before it threw, so that case promises nothing.
      title = "This slop's view stopped"
      explanation =
        "The view that shows this slop quit unexpectedly. Reopening it usually fixes this. "
        + "Edits you already made are kept; text you were still typing may be lost. If it keeps happening, please tell us."
    default:
      title = "This slop stopped responding"
      explanation = "Something went wrong while showing this slop. Try reopening it, and tell us if it keeps happening."
    }
    details = message
    report = """
      hitSlop failure report
      hitSlop: \(host.app)
      Core build: \(host.core)
      System: \(host.system), \(host.architecture)
      Slop: \(slopTitle) (\(slopSlug))
      Reason: \(reason?.rawValue ?? "unknown")
      Classification: \(classification.rawValue)

      \(Self.excerpt(message))
      """
  }

  static func excerpt(_ message: String) -> String {
    let lines = message.split(separator: "\n", omittingEmptySubsequences: false).prefix(excerptLines)
    let text = String(lines.joined(separator: "\n").prefix(excerptCharacters))
    return text == message ? text : text + "\n…"
  }
}
