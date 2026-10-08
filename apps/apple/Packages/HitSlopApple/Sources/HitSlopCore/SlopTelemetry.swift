import Foundation
import HitSlopCoreBinding

/// PNG or PDF: the socket's export format, also used by the window and telemetry.
public typealias ExportFormat = WireExportFormat
extension WireExportFormat {
  public var fileExtension: String {
    switch self {
    case .png: "png"
    case .pdf: "pdf"
    }
  }
}

/// Only fixed product events cross this boundary; document values never enter telemetry.
public enum SlopTelemetryEvent: Equatable, Sendable {
  public enum TemplateSource: String, Sendable { case bundled, installed }
  public enum Failure: String, Sendable {
    case create, open, save, export, renderer, duplicate, close, quit, recovery, catalog, artwork
    case themeImport, themeExport, share
    // Explicit identifiers are persistent Crashlytics grouping keys. Never renumber.
    public var code: Int {
      switch self {
      case .create: 1
      case .open: 2
      case .save: 3
      case .export: 4
      case .renderer: 5
      case .duplicate: 6
      case .close: 7
      case .quit: 8
      case .recovery: 9
      case .catalog: 10
      case .artwork: 11
      case .themeImport: 12
      case .themeExport: 13
      case .share: 14
      }
    }
  }
  public enum Phase: String, Sendable { case started, completed, cancelled, failed, recovered }
  case launched, opened, duplicated
  case themeEditorOpened, themeImported, themeExported
  case created(TemplateSource)
  case exported(ExportFormat)
  case failed(Failure, SlopFailureContext = .init())
  case breadcrumb(Failure, Phase)
}

public struct SlopFailureContext: Equatable, Sendable {
  public enum Classification: String, Sendable {
    case platform, authored, rejection
    var code: Int {
      switch self {
      case .platform: 1
      case .authored: 2
      case .rejection: 3
      }
    }
  }
  public enum Reason: String, Sendable {
    case unknown, storage, webContentTerminated, navigation, startup, presentation
    // Reported under its earlier name, so the analytics series continues.
    case invalidFile = "invalidPackage"
    case missingFile, permission, diskFull, busy
    case authoredException, operationRejected, preview, icon, destinationExists, requiresUpdate
    var code: Int {
      switch self {
      case .unknown: 0
      case .storage: 1
      case .webContentTerminated: 2
      case .navigation: 3
      case .startup: 4
      case .presentation: 5
      case .invalidFile: 6
      case .missingFile: 7
      case .permission: 8
      case .diskFull: 9
      case .busy: 10
      case .authoredException: 12
      case .operationRejected: 13
      // 16 was retired; codes are never reused.
      case .preview: 14
      case .icon: 15
      case .destinationExists: 17
      case .requiresUpdate: 18
      }
    }
  }
  public let classification: Classification
  public let reason: Reason
  public var format: ExportFormat?
  public init(
    _ classification: Classification = .platform, reason: Reason = .unknown,
    format: ExportFormat? = nil
  ) {
    self.classification = classification
    self.reason = reason
    self.format = format
  }
  public static func classify(_ error: Error) -> Self {
    if let diagnostic = error as? any SlopDiagnosticProviding { return diagnostic.diagnostic }
    // The core never replaces a file: creating, duplicating or sharing onto one is refused.
    if let failure = error as? OwnerFailure, failure.refusal == .exists {
      return .init(.rejection, reason: .destinationExists)
    }
    // Never serialize the original NSError, its description, or userInfo.
    let native = error as NSError
    if native.domain == NSCocoaErrorDomain {
      switch native.code {
      case NSFileNoSuchFileError, NSFileReadNoSuchFileError: return .init(reason: .missingFile)
      case NSFileReadNoPermissionError, NSFileWriteNoPermissionError: return .init(reason: .permission)
      case NSFileWriteFileExistsError: return .init(.rejection, reason: .destinationExists)
      case NSFileWriteOutOfSpaceError: return .init(reason: .diskFull)
      default: break
      }
    }
    if native.domain == NSPOSIXErrorDomain {
      switch native.code {
      case 2: return .init(reason: .missingFile)
      case 1, 13: return .init(reason: .permission)
      case 28: return .init(reason: .diskFull)
      default: break
      }
    }
    return .init()
  }
  public static func isCancellation(_ error: Error) -> Bool {
    if error is CancellationError { return true }
    let native = error as NSError
    return (native.domain == NSCocoaErrorDomain && native.code == NSUserCancelledError)
      || (native.domain == NSURLErrorDomain && native.code == NSURLErrorCancelled)
  }
  public func code(for operation: SlopTelemetryEvent.Failure) -> Int {
    operation.code * 10_000 + classification.code * 100 + reason.code
  }
  /// This is the complete allowlist of values permitted in an uploaded non-fatal.
  public func fields(for operation: SlopTelemetryEvent.Failure) -> [String: String] {
    var fields = [
      "operation": operation.rawValue, "classification": classification.rawValue, "reason": reason.rawValue,
    ]
    if let format { fields["format"] = format.fileExtension }
    return fields
  }
}

public protocol SlopDiagnosticProviding: Error { var diagnostic: SlopFailureContext { get } }

/// Local errors retain their existing UI text; only the fixed diagnostic crosses telemetry.
public struct SlopDiagnosticError: LocalizedError, SlopDiagnosticProviding {
  public let underlying: Error
  public let diagnostic: SlopFailureContext
  public init(_ underlying: Error, diagnostic: SlopFailureContext) {
    self.underlying = underlying
    self.diagnostic = diagnostic
  }
  public var errorDescription: String? { underlying.localizedDescription }
}

/// One instance per application launch. Low-priority reports cannot exhaust Crashlytics' history.
@MainActor public final class SlopTelemetryPolicy {
  private var secondaryCodes = Set<Int>()
  private var foregroundFailed = false
  public init() {}
  public func shouldRecord(_ operation: SlopTelemetryEvent.Failure, context: SlopFailureContext) -> Bool {
    let secondary = context.classification != .platform || operation == .catalog || operation == .artwork
    if !secondary {
      foregroundFailed = true
      return true
    }
    guard !foregroundFailed, secondaryCodes.count < 2 else { return false }
    return secondaryCodes.insert(context.code(for: operation)).inserted
  }
}

@MainActor public struct SlopTelemetry {
  private let receive: (SlopTelemetryEvent) -> Void
  public init(send: @escaping (SlopTelemetryEvent) -> Void) { self.receive = send }
  public func send(_ event: SlopTelemetryEvent) { receive(event) }
  public func failure(
    _ operation: SlopTelemetryEvent.Failure, error: Error,
    format: ExportFormat? = nil
  ) {
    if SlopFailureContext.isCancellation(error) {
      send(.breadcrumb(operation, .cancelled))
      return
    }
    var context = SlopFailureContext.classify(error)
    context.format = format
    send(.breadcrumb(operation, .failed))
    send(.failed(operation, context))
  }
  public static let disabled = Self { _ in }
}
