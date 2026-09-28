import AppKit
import Foundation
import Testing
import HitSlopHost
import HitSlopRuntime

@MainActor private func withPresentationSession(
    _ kind: String = "standard", body: (SlopRuntimeSession) async throws -> Void
) async throws {
    let source = try #require(ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"])
    let paths = try JSONDecoder().decode([String: String].self, from: Data(source.utf8))
    let path = try #require(paths[kind])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(at: URL(fileURLWithPath: path), to: root)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await SlopRuntimeSession.open(packageURL: root)
    session.load()
    do {
        try await session.waitUntilReady()
        try await body(session)
    } catch {
        try await session.finish()
        throw error
    }
    try await session.finish()
}

extension LoroClientTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func captureRestoresAnOperableEditor() async throws {
        for kind in ["standard", "ellipse", "washer"] {
            try await withPresentationSession(kind) { (session: SlopRuntimeSession) async throws in
                let view = session.webView
                let data = try await SlopRenderer.exportPNGData(session: session)
                #expect(NSImage(data: data) != nil)
                #expect(try await view.callAsyncJavaScript("""
                    const button = document.querySelector('.editor button'), before = button.textContent;
                    button.click();
                    await globalThis.__slop.flush();
                    return button.isConnected && !button.disabled && button.textContent !== before &&
                        !document.documentElement.hasAttribute('data-slop-capture');
                    """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true)
            }
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func snippetFailurePreservesEditorAndAllowsCaptureRetry() async throws {
        try await withPresentationSession { (session: SlopRuntimeSession) async throws in
            var issues: [String] = []
            session.engine.onIssue = { message, _ in issues.append(message) }
            for kind in ["export", "icon"] {
                _ = try await session.webView.evaluateJavaScript(
                    "globalThis.__presentationFailure = {kind:'\(kind)',error:new Error('Snippet failed')}; true")
                await #expect(throws: (any Error).self) {
                    if kind == "icon" {
                        _ = try await SlopRenderer.targetPNGData(session: session, target: .icon)
                    } else {
                        _ = try await SlopRenderer.exportPNGData(session: session)
                    }
                }
                #expect(try await session.webView.callAsyncJavaScript("""
                    const button = document.querySelector('.editor button'), before = button.textContent;
                    button.click();
                    await globalThis.__slop.flush();
                    return button.isConnected && button.textContent !== before &&
                        !document.querySelector('[role="alert"]') &&
                        !document.documentElement.hasAttribute('data-slop-capture') &&
                        [...document.querySelectorAll('[data-slop-capture-target]')].every(e => e.hidden && !e.childElementCount);
                    """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true)
                #expect(issues.isEmpty)
                _ = try await session.webView.evaluateJavaScript("delete globalThis.__presentationFailure; true")
                let data = kind == "icon"
                    ? try await SlopRenderer.targetPNGData(session: session, target: .icon)
                    : try await SlopRenderer.exportPNGData(session: session)
                #expect(NSImage(data: try #require(data)) != nil)
            }
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func falsyEditorFailuresRejectReloadAndCapture() async throws {
        try await withPresentationSession { (session: SlopRuntimeSession) async throws in
            var recoveries = 0
            session.engine.onRecovered = { recoveries += 1 }
            for value in ["null", "undefined", "false", "0", "''"] {
                let before = recoveries
                #expect(try await session.webView.callAsyncJavaScript("""
                    globalThis.__presentationFailure = {kind:'editor',error:\(value)};
                    let rejected = false;
                    try { await __slop.reloadInterface(); } catch { rejected = true; }
                    if (!rejected || !document.querySelector('[role="alert"]')) return false;
                    const capture = globalThis.__hitslopCapture;
                    try { await capture.begin('falsy-test', 'export'); return false; }
                    catch { return !document.documentElement.hasAttribute('data-slop-capture'); }
                    finally { await capture.restore('falsy-test'); }
                    """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true)
                #expect(recoveries == before)
                _ = try await session.webView.callAsyncJavaScript(
                    "delete globalThis.__presentationFailure; await __slop.reloadInterface(); return true",
                    arguments: [:], in: nil, contentWorld: .page)
                #expect(recoveries == before + 1)
            }
        }
    }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil,
              "Run bun run swift:test to build presentation fixtures"))
@MainActor func presentationFixturesExportWithoutNativeMaskAndRenderSquareIcons() async throws {
    let source = try #require(ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"])
    let paths = try JSONDecoder().decode([String: String].self, from: Data(source.utf8))
    #expect(Set(paths.keys) == Set(["standard", "ellipse", "washer"]))
    for path in paths.values {
        let url = URL(fileURLWithPath: path)
        let exported = try await SlopRenderer.exportPNGData(packageURL: url)
        let bitmap = try #require(NSBitmapImageRep(data: exported))
        #expect(bitmap.pixelsWide == 640)
        #expect(bitmap.pixelsHigh == 480)
        // Dedicated export corners remain opaque even for an ellipse or holed skin.
        #expect((bitmap.colorAt(x: 2, y: 2)?.alphaComponent ?? 0) > 0.99)
        let icon = try #require(await SlopRenderer.targetPNGData(packageURL: url, target: .icon))
        let image = try #require(NSBitmapImageRep(data: icon))
        #expect(image.pixelsWide == 512)
        #expect(image.pixelsHigh == 512)
        #expect((image.colorAt(x: 256, y: 256)?.alphaComponent ?? 1) == 0)
        #expect((image.colorAt(x: 256, y: 80)?.alphaComponent ?? 0) > 0.99)
        // The fixture's opaque ring is red; preserve that color across native profiles.
        let ring = try #require(image.colorAt(x: 256, y: 80)?.usingColorSpace(.sRGB))
        #expect(ring.redComponent > 0.8)
        #expect(ring.redComponent > ring.greenComponent + 0.5)
        #expect(ring.redComponent > ring.blueComponent + 0.5)
        #expect((image.colorAt(x: 256, y: 431)?.alphaComponent ?? 0) > 0.99)
        #expect((image.colorAt(x: 256, y: 30)?.alphaComponent ?? 1) == 0)
    }
}
