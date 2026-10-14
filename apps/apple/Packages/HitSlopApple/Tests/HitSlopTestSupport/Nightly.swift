import Foundation
import Testing

extension Trait where Self == ConditionTrait {
  /// Nightly and release only (`HITSLOP_NIGHTLY=1`, set by `verify --release` and the nightly
  /// CI run): presentation, pixel and telemetry checks that never guard opening, editing or
  /// saving a document. Every change runs the rest (docs/testing.md).
  public static var nightly: Self {
    .enabled(if: ProcessInfo.processInfo.environment["HITSLOP_NIGHTLY"] == "1", "Nightly and release only")
  }
}
