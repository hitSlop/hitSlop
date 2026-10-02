import AppKit
import Foundation
import PDFKit
import Testing
import HitSlopTestSupport
@testable import HitSlopHost
import HitSlopDocument

@MainActor private func withPresentationSession(
    _ kind: String = "standard", body: (DocumentSession) async throws -> Void
) async throws {
    let source = try #require(ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"])
    let paths = try JSONDecoder().decode([String: String].self, from: Data(source.utf8))
    let path = try #require(paths[kind])
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(at: URL(fileURLWithPath: path), to: root)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = try await DocumentSession.open(packageURL: root)
    session.load()
    do {
        try await session.waitUntilReady()
        try await body(session)
    } catch {
        try await session.close()
        throw error
    }
    try await session.close()
}

extension OwnerClientTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func shapeLabNativeResizeHonorsAspectAndKeepsEditorOperable() async throws {
        let source = try #require(ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"])
        let paths = try JSONDecoder().decode([String: String].self, from: Data(source.utf8))
        for kind in ["hole", "locked"] {
            let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
            try FileManager.default.copyItem(at: URL(fileURLWithPath: try #require(paths["shape-lab-" + kind])), to: root)
            defer { try? FileManager.default.removeItem(at: root) }
            let controller = try await SlopDocumentWindowController.open(packageURL: root)
            do {
                try await controller.session.waitUntilReady()
                let window = try #require(controller.window)
                let resized = try await controller.session.webView.callAsyncJavaScript("""
                    [...document.querySelectorAll('button')].find(b => b.textContent === '600 × 400').click();
                    for (let i = 0; i < 100; i++) {
                        await new Promise(resolve => setTimeout(resolve, 20));
                        if (innerWidth === 600) return true;
                    }
                    return false;
                    """, arguments: [:], in: nil, contentWorld: .page)
                #expect(resized as? Bool == true)
                let size = try #require(window.contentView).bounds.size
                #expect(abs(size.width - 600) < 1)
                #expect(abs(size.height - (kind == "locked" ? 450 : 400)) < 1)
                #expect(try await controller.session.webView.callAsyncJavaScript("""
                    document.querySelector('.shape-lab-west').click();
                    await __slop.flush();
                    return document.querySelector('.shape-lab-readout strong').textContent === '1';
                    """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true)
                try await controller.session.close()
                controller.close()
            } catch {
                try await controller.session.close()
                controller.close()
                throw error
            }
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func captureRestoresAnOperableEditor() async throws {
        for kind in ["standard", "ellipse", "washer"] {
            try await withPresentationSession(kind) { (session: DocumentSession) async throws in
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
        try await withPresentationSession { (session: DocumentSession) async throws in
            var issues: [String] = []
            let events = SessionEvents()
            events.issue = { issues.append($0.message) }
            session.delegate = events
            for kind in ["export", "icon"] {
                _ = try await session.webView.evaluateJavaScript(
                    "globalThis.__presentationFailure = {kind:'\(kind)',error:new Error('Snippet failed')}; true")
                await #expect(throws: (any Error).self) {
                    if kind == "icon" {
                        _ = try await SlopRenderer.iconPNGData(session: session)
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
                    ? try await SlopRenderer.iconPNGData(session: session)
                    : try await SlopRenderer.exportPNGData(session: session)
                #expect(NSImage(data: try #require(data)) != nil)
            }
        }
    }

    // Failure: a refused write the app left unhandled reached the window as an authored
    // failure, and WebKit's frames-only stack hid its message.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil),
          .timeLimit(.minutes(1)))
    @MainActor func unhandledRefusalIsReportedAsAnOperationWithItsMessage() async throws {
        try await withPresentationSession { (session: DocumentSession) async throws in
            let (reported, report) = AsyncStream.makeStream(of: SlopPageIssue.self)
            let events = SessionEvents()
            events.issue = { report.yield($0) }
            session.delegate = events
            _ = try await session.webView.callAsyncJavaScript("""
                globalThis.__presentationIncrement = 0;
                document.querySelector('.editor button').click();
                """, arguments: [:], in: nil, contentWorld: .page)
            var issues = reported.makeAsyncIterator()
            let issue = try #require(await issues.next())
            #expect(issue.isOperation)
            #expect(issue.message.hasPrefix("DocumentError: "))
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func falsyEditorFailuresRejectReloadAndCapture() async throws {
        try await withPresentationSession { (session: DocumentSession) async throws in
            var recoveries = 0
            let events = SessionEvents()
            events.recovered = { recoveries += 1 }
            session.delegate = events
            for value in ["null", "undefined", "false", "0", "''"] {
                let before = recoveries
                #expect(try await session.webView.callAsyncJavaScript("""
                    globalThis.__presentationFailure = {kind:'editor',error:\(value)};
                    let rejected = false;
                    try { await __slop.reloadInterface(); } catch { rejected = true; }
                    if (!rejected || !document.querySelector('[role="alert"]')) return false;
                    const capture = globalThis.__slop.capture;
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
    #expect(Set(["standard", "ellipse", "washer"]).isSubset(of: Set(paths.keys)))
    for kind in ["standard", "ellipse", "washer"] {
        let path = try #require(paths[kind])
        let url = URL(fileURLWithPath: path)
        let exported = try await SlopRenderer.exportPNGData(packageURL: url)
        let bitmap = try #require(NSBitmapImageRep(data: exported))
        #expect(bitmap.pixelsWide == 640)
        #expect(bitmap.pixelsHigh == 480)
        // Dedicated export corners remain opaque even for an ellipse or holed skin.
        #expect((bitmap.colorAt(x: 2, y: 2)?.alphaComponent ?? 0) > 0.99)
        let icon = try #require(await SlopRenderer.iconPNGData(packageURL: url))
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


private func shapeLabKinds(fallback: Bool) throws -> [String] {
    let source = try #require(ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"])
    let paths = try JSONDecoder().decode([String: String].self, from: Data(source.utf8))
    let kinds = paths.keys.filter { $0.hasPrefix("shape-lab-") && $0.hasSuffix("-fallback") == fallback }.sorted()
    #expect(kinds.count == 6)
    return kinds
}

extension OwnerClientTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func shapeLabCapturesPreserveCornersAndAcceptedInput() async throws {
        for kind in try shapeLabKinds(fallback: false) {
            try await withPresentationSession(kind) { session in
                let accepted = try await session.webView.callAsyncJavaScript("""
                    document.querySelector('.shape-lab-west').click();
                    const input = document.querySelector('input[aria-label="Capture note"]');
                    input.value = 'Typed before capture';
                    input.dispatchEvent(new Event('input', { bubbles: true }));
                    await __slop.flush();
                    return document.querySelector('.shape-lab-readout strong').textContent === '1';
                    """, arguments: [:], in: nil, contentWorld: .page)
                #expect(accepted as? Bool == true)
                let png = try await SlopRenderer.exportPNGData(session: session)
                let image = try #require(NSBitmapImageRep(data: png))
                #expect(image.pixelsWide == (kind.contains("washer") ? 640 : 960))
                for (x, y) in [(2, 2), (image.pixelsWide - 3, 2), (2, image.pixelsHigh - 3), (image.pixelsWide - 3, image.pixelsHigh - 3)] {
                    let color = try #require(image.colorAt(x: x, y: y)?.usingColorSpace(.sRGB))
                    #expect(color.alphaComponent > 0.99)
                    #expect(color.redComponent > color.greenComponent + 0.2)
                }
                let pdf = try #require(PDFDocument(data: await SlopRenderer.exportPDFData(session: session)))
                #expect(pdf.string?.contains("Typed before capture") == true)
                #expect(pdf.page(at: 0)?.bounds(for: .mediaBox).width == (kind.contains("washer") ? 320 : 480))
                let icon = try #require(await SlopRenderer.iconPNGData(session: session))
                let square = try #require(NSBitmapImageRep(data: icon))
                #expect(square.pixelsWide == 512 && square.pixelsHigh == 512)
                #expect((square.colorAt(x: 2, y: 2)?.alphaComponent ?? 0) > 0.99)
                #expect(try await session.webView.callAsyncJavaScript("""
                    const input = document.querySelector('input[aria-label="Capture note"]');
                    document.querySelector('.shape-lab-east').click();
                    await __slop.flush();
                    return input.value === 'Typed before capture' &&
                      document.querySelector('.shape-lab-readout strong').textContent === '2' &&
                      !document.documentElement.hasAttribute('data-slop-capture');
                    """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true)
            }
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func shapeLabFallbackCapturesKeepNativeMask() async throws {
        for kind in try shapeLabKinds(fallback: true) {
            try await withPresentationSession(kind) { session in
                let preview = try await SlopRenderer.previewPNGData(session: session)
                let small = try #require(NSBitmapImageRep(data: preview))
                #expect(small.pixelsWide == (kind.contains("washer") ? 320 : 480))
                let png = try await SlopRenderer.exportPNGData(session: session)
                let image = try #require(NSBitmapImageRep(data: png))
                #expect(image.pixelsWide == small.pixelsWide * 2)
                for bitmap in [small, image] {
                    #expect((bitmap.colorAt(x: 0, y: 0)?.alphaComponent ?? 1) == 0)
                    let scale = Double(bitmap.pixelsWide) / 480
                    if kind.contains("hole") || kind.contains("locked") {
                        // SVG coordinates are top-down. The mirrored point must remain opaque.
                        #expect((bitmap.colorAt(x: Int(350 * scale), y: Int(88 * scale))?.alphaComponent ?? 1) == 0)
                        #expect((bitmap.colorAt(x: Int(350 * scale), y: Int(272 * scale))?.alphaComponent ?? 0) > 0.99)
                    }
                    if kind.contains("concave") {
                        #expect((bitmap.colorAt(x: Int(465 * scale), y: Int(156 * scale))?.alphaComponent ?? 1) == 0)
                        #expect((bitmap.colorAt(x: Int(420 * scale), y: Int(156 * scale))?.alphaComponent ?? 0) > 0.99)
                    }
                    let x = bitmap.pixelsWide / 2, y = bitmap.pixelsHigh / 2;
                    if kind.contains("washer") { #expect((bitmap.colorAt(x: x, y: y)?.alphaComponent ?? 1) == 0) }
                }
                let centre = image.colorAt(x: image.pixelsWide / 2, y: image.pixelsHigh / 2)?.alphaComponent ?? -1
                if kind.contains("washer") { #expect(centre == 0) }
                else { #expect(centre > 0.99) }
            }
        }
    }
}

extension OwnerClientTests {
    // Failure: a full-length fallback PNG export was clipped by the window silhouette
    // stretched to the export's height. The silhouette describes the window: only a
    // window-sized capture is masked, and longer exports are unmasked, like PDF.
    @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_PRESENTATION_FIXTURES"] != nil))
    @MainActor func longFallbackExportsAreNotMaskedByTheWindowSilhouette() async throws {
        let kind = try #require(try shapeLabKinds(fallback: true).first { $0.contains("rounded") })
        try await withPresentationSession(kind) { session in
            _ = try await session.webView.callAsyncJavaScript("""
                const tall = document.createElement('div');
                tall.style.height = '1600px';
                document.querySelector('[data-hitslop-root]').appendChild(tall);
                return true;
                """, arguments: [:], in: nil, contentWorld: .page)
            let png = try await SlopRenderer.exportPNGData(session: session)
            let image = try #require(NSBitmapImageRep(data: png))
            #expect(image.pixelsHigh > 2 * 360 * 2)
            #expect((image.colorAt(x: 0, y: 0)?.alphaComponent ?? 0) > 0.99)
            #expect((image.colorAt(x: 0, y: image.pixelsHigh - 1)?.alphaComponent ?? 0) > 0.99)
        }
    }
}
