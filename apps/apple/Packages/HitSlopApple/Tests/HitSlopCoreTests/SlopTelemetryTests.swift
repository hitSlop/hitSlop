import Foundation
import Testing
@testable import HitSlopCore

// New boundary: incidental NSError strings must never become report keys; cancellations
// must not count as failures. Existing host tests did not inspect the payload allowlist.
@Test @MainActor func telemetryDropsPrivateErrorFieldsAndCancellation() {
    var events: [SlopTelemetryEvent] = []
    let telemetry = SlopTelemetry { events.append($0) }
    telemetry.failure(.duplicate, error: NSError(domain: NSCocoaErrorDomain,
        code: NSFileWriteNoPermissionError, userInfo: [NSLocalizedDescriptionKey: "secret title",
        NSFilePathErrorKey: "/private/example/document.slop", "arbitrary": "authored contents"]))
    let contexts = events.compactMap { event -> SlopFailureContext? in
        if case .failed(_, let context) = event { return context }; return nil
    }
    #expect(contexts.count == 1)
    #expect(contexts.first?.fields(for: .duplicate) == [
        "operation": "duplicate", "classification": "platform", "reason": "permission"])
    // Persistent grouping must not change with a different private message or pathname.
    #expect(contexts.first?.code(for: .duplicate) == 60_108)
    events.removeAll()
    telemetry.failure(.export, error: CancellationError())
    telemetry.failure(.create, error: CocoaError(.userCancelled))
    #expect(events == [.breadcrumb(.export, .cancelled), .breadcrumb(.create, .cancelled)])
}

// New policy: repeating guest/catalog problems must not displace foreground failures from
// Crashlytics' eight-event history. Test independent counts and launch-local reset.
@Test @MainActor func secondaryReportsAreBoundedAcrossDocumentsAndStopAfterPlatformFailure() {
    let policy = SlopTelemetryPolicy()
    let authored = SlopFailureContext(.authored, reason: .authoredException)
    let rejection = SlopFailureContext(.rejection, reason: .invalidFile)
    #expect(policy.shouldRecord(.renderer, context: authored))
    #expect(!policy.shouldRecord(.renderer, context: authored))
    #expect(policy.shouldRecord(.open, context: rejection))
    #expect(!policy.shouldRecord(.artwork, context: .init(reason: .icon)))
    #expect(policy.shouldRecord(.save, context: .init(reason: .storage)))
    #expect(policy.shouldRecord(.renderer, context: .init(reason: .webContentTerminated)))
    let nextLaunch = SlopTelemetryPolicy()
    #expect(nextLaunch.shouldRecord(.save, context: .init(reason: .storage)))
    #expect(!nextLaunch.shouldRecord(.renderer, context: authored))
    #expect(!nextLaunch.shouldRecord(.catalog, context: .init()))
    #expect(SlopTelemetryPolicy().shouldRecord(.renderer, context: authored))
}
