import PopCore
import PopDocument

/// One report per save/renderer incident and authored issue kind, with recovery breadcrumbs.
@MainActor final class WindowFailureReporter {
  private enum IssueKind { case operation, authored }
  private var issues = Set<IssueKind>()
  private var saveFailed = false
  private var rendererFailed = false

  func renderer(_ diagnostic: PopFailureContext, telemetry: PopTelemetry) {
    guard !rendererFailed else { return }
    rendererFailed = true
    telemetry.send(.breadcrumb(.renderer, .failed))
    if !saveFailed || diagnostic.reason == .webContentTerminated || diagnostic.reason == .unresponsive {
      telemetry.send(.failed(.renderer, diagnostic))
    }
  }
  func rendererRecovered(telemetry: PopTelemetry) {
    if rendererFailed { telemetry.send(.breadcrumb(.renderer, .recovered)) }
    rendererFailed = false
  }
  func issue(_ issue: PopPageIssue, telemetry: PopTelemetry) {
    let kind: IssueKind = issue.isOperation ? .operation : .authored
    guard issues.insert(kind).inserted else { return }
    telemetry.send(
      .failed(
        .renderer,
        .init(
          issue.isOperation ? .rejection : .authored,
          reason: issue.isOperation ? .operationRejected : .authoredException)))
  }
  func save(_ status: DocumentSaveStatus, telemetry: PopTelemetry) {
    switch status {
    case .failed:
      if !saveFailed {
        telemetry.send(.breadcrumb(.save, .failed))
        telemetry.send(.failed(.save, .init(reason: .storage)))
      }
      saveFailed = true
    case .saved:
      if saveFailed { telemetry.send(.breadcrumb(.save, .recovered)) }
      saveFailed = false
    case .saving: break
    }
  }
  func lifecycle(_ operation: PopTelemetryEvent.Failure, error: Error, format: ExportFormat?, telemetry: PopTelemetry) {
    if PopFailureContext.isCancellation(error) {
      telemetry.send(.breadcrumb(operation, .cancelled))
    } else if saveFailed || rendererFailed {
      telemetry.send(.breadcrumb(operation, .failed))
    } else {
      telemetry.failure(operation, error: error, format: format)
    }
  }
}
