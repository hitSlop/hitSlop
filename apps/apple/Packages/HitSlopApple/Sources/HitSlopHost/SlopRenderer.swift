import AppKit
import Foundation
import HitSlopCore
import HitSlopDocument
import WebKit

struct SlopDocumentAssets: Sendable {
    let previewPNG: Data?
    let finderIconPNG: Data?
}

@MainActor public enum SlopRenderer {
    private enum CaptureOutput { case previewPNG, exportPNG, pdf }
    private static var waiters: [ObjectIdentifier: [CheckedContinuation<Void, Never>]] = [:]

    /// One capture at a time per session. `capturing` stays set across a hand-off to a
    /// queued capture, so no page or socket request slips in between.
    private static func acquire(_ session: DocumentSession) async {
        let key = ObjectIdentifier(session)
        if waiters[key] == nil { waiters[key] = [] }
        else { await withCheckedContinuation { waiters[key, default: []].append($0) } }
        session.capturing = true
    }
    private static func release(_ session: DocumentSession) {
        let key = ObjectIdentifier(session)
        if waiters[key]?.isEmpty == false { waiters[key]?.removeFirst().resume() }
        else { waiters.removeValue(forKey: key); session.capturing = false }
    }

    public static func previewPNGData(packageURL: URL) async throws -> Data { try await render(packageURL: packageURL, output: .previewPNG) }
    // Test entrypoints: the CLI exports live owners or uses `withRenderSession` directly.
    static func exportPNGData(packageURL: URL) async throws -> Data { try await render(packageURL: packageURL, output: .exportPNG) }
    static func exportPDFData(packageURL: URL) async throws -> Data { try await render(packageURL: packageURL, output: .pdf) }
    public static func previewPNGData(session: DocumentSession) async throws -> Data { try await capture(session: session, output: .previewPNG) }
    public static func exportPNGData(session: DocumentSession) async throws -> Data { try await capture(session: session, output: .exportPNG) }
    public static func exportPDFData(session: DocumentSession) async throws -> Data { try await capture(session: session, output: .pdf) }

    static func documentAssetsPNGData(packageURL: URL, telemetry: SlopTelemetry = .disabled) async throws -> SlopDocumentAssets {
        try await withRenderSession(packageURL: packageURL, renderTargetsEnabled: true) { session in
            var preview: Data?, icon: Data?
            do { preview = try await capture(session: session, output: .previewPNG) }
            catch { if !SlopFailureContext.isCancellation(error) { telemetry.send(.failed(.artwork, .init(reason: .preview))) } }
            try Task.checkCancellation()
            do { icon = try await iconPNGData(session: session) }
            catch { if !SlopFailureContext.isCancellation(error) { telemetry.send(.failed(.artwork, .init(reason: .icon))) } }
            try Task.checkCancellation()
            return SlopDocumentAssets(previewPNG: preview, finderIconPNG: icon)
        }
    }

    /// Background renders read the saved document into memory storage. They take
    /// no ownership and never write inside the package, so no copy is needed.
    /// `inputReady` runs once that snapshot exists (or opening failed), before rendering.
    static func withRenderSession<T>(
        packageURL: URL, renderTargetsEnabled: Bool = false,
        inputReady: @MainActor () -> Void = {},
        _ capture: @MainActor (DocumentSession) async throws -> T
    ) async throws -> T {
        let session: DocumentSession
        do {
            session = try await DocumentSession.open(
                packageURL: packageURL, renderTargetsEnabled: renderTargetsEnabled, purpose: .backgroundRender)
        } catch {
            inputReady()
            throw error
        }
        inputReady()
        let window = hiddenWindow(session)
        let result: Result<T, Error>
        do {
            session.load()
            try await session.waitUntilReady()
            try Task.checkCancellation()
            result = .success(try await capture(session))
        } catch { result = .failure(error) }
        window.contentView = nil
        try await session.close()
        return try result.get()
    }

    private static func hiddenWindow(_ session: DocumentSession) -> NSWindow {
        let window = NSWindow(contentRect: session.webView.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = session.webView
        window.orderOut(nil)
        return window
    }

    public static func iconPNGData(packageURL: URL) async throws -> Data? {
        try await withRenderSession(packageURL: packageURL, renderTargetsEnabled: true) { session in
            try await iconPNGData(session: session)
        }
    }

    public static func iconPNGData(session: DocumentSession) async throws -> Data? {
        try await withCapture(session) { view, token, originalFrame in
            view.frame.size = CGSize(width: max(512, originalFrame.width), height: max(512, originalFrame.height))
            let value = try await begin(view, token: token, mode: "icon")
            guard value["dedicated"] as? Bool == true else { return nil }
            let rect = try geometry(value)
            guard rect.width > 0, abs(rect.width - rect.height) < 0.5,
                  rect.minX >= -0.5, rect.minY >= -0.5,
                  rect.maxX <= view.bounds.width + 0.5, rect.maxY <= view.bounds.height + 0.5 else {
                throw SlopPackageError.invalid("icon target must be a visible square inside the capture viewport")
            }
            WebViewBackground.set(false, on: view)
            let configuration = WKSnapshotConfiguration()
            configuration.rect = rect
            configuration.snapshotWidth = 512
            return try SlopPreviewImage.png(from: try await view.takeSnapshot(configuration: configuration))
        }
    }

    /// One capture of a session: it waits for exclusive use, saves pending edits, and
    /// afterward restores the frame, the page's capture state and the background, whatever
    /// `body` did. `body` begins the page's capture mode itself.
    private static func withCapture<T>(
        _ session: DocumentSession,
        _ body: (_ view: WKWebView, _ token: String, _ originalFrame: CGRect) async throws -> T
    ) async throws -> T {
        await acquire(session)
        defer { release(session) }
        try await session.flush()
        try Task.checkCancellation()
        let view = session.webView, token = UUID().uuidString, originalFrame = view.frame
        let background = WebViewBackground.get(view)
        defer { WebViewBackground.set(background, on: view) }
        do {
            let result = try await body(view, token, originalFrame)
            restoreFrame(view, original: originalFrame)
            try await restore(view, token: token)
            try Task.checkCancellation()
            return result
        } catch {
            restoreFrame(view, original: originalFrame)
            try? await restore(view, token: token)
            throw captureFailure(error)
        }
    }

    private static func render(packageURL: URL, output: CaptureOutput) async throws -> Data {
        try await withRenderSession(packageURL: packageURL) { session in
            try await capture(session: session, output: output)
        }
    }

    private static func capture(session: DocumentSession, output: CaptureOutput) async throws -> Data {
        let isPreview = output == .previewPNG
        return try await withCapture(session) { view, token, originalFrame in
            var measurement = try await begin(view, token: token, mode: isPreview ? "preview" : "export")
            let dedicated = measurement["dedicated"] as? Bool == true
            var width = originalFrame.width
            var height = isPreview ? originalFrame.height : max(dedicated ? 1 : originalFrame.height, try geometry(measurement).height)
            var rect = CGRect(x: 0, y: 0, width: width, height: height)
            // Dedicated exports are the object itself, independent of the native window mask.
            // Re-measure after resizing; reject viewport-dependent layouts that never settle.
            // PDF captures offscreen content directly; avoid an enormous backing view.
            if isPreview && dedicated {
                // Previews show the top of long exports: at most 3:1 portrait (or the window height).
                // Designed compositions stay whole; PNG/PDF export keeps the full length.
                func previewHeight(_ box: CGRect) -> CGFloat { max(originalFrame.height, 3 * max(box.width, 1)) }
                var box = try geometry(measurement)
                var viewWidth = originalFrame.width
                var viewHeight = originalFrame.height
                var settled = false
                for _ in 0..<4 {
                    if max(0, box.width - viewWidth, ceil(box.maxX) - viewWidth) >= 48 {
                        viewWidth = max(viewWidth, box.width, ceil(box.maxX))
                    }
                    let heightLimit = max(originalFrame.height, ceil(max(0, box.minY)) + previewHeight(box))
                    if max(0, box.height - viewHeight, ceil(box.maxY) - viewHeight) >= 1 {
                        viewHeight = min(max(viewHeight, box.height, ceil(box.maxY)), heightLimit)
                    }
                    try validateSize(width: max(min(box.width, viewWidth), 1), height: max(min(box.height, viewHeight), 1), output: output, scale: 2)
                    try await resizeAndSettle(view, to: CGSize(width: viewWidth, height: viewHeight), token: token, measurement: &measurement)
                    let next = try geometry(measurement)
                    let bounds = CGRect(x: 0, y: 0, width: viewWidth, height: viewHeight)
                    let visible = next.intersection(bounds)
                    // Content continuing below a height-limited viewport is expected.
                    if next.minX >= -0.5, next.minY >= -0.5,
                       next.maxX <= viewWidth + 48, next.maxY <= viewHeight + 48 || viewHeight >= heightLimit,
                       visible.width >= 8, visible.height >= 8 {
                        settled = true
                        box = visible
                        break
                    }
                    box = next
                }
                guard settled else { throw SlopPackageError.invalid("Preview export layout keeps changing with viewport size; use normal flow in Export.svelte") }
                width = max(box.width, 1)
                height = max(min(box.height, previewHeight(box)), 1)
                rect = CGRect(x: box.minX, y: box.minY, width: width, height: height)
            } else if output == .exportPNG {
                var settled = false
                for _ in 0..<4 {
                    try validateSize(width: width, height: height, output: output, scale: 2)
                    try await resizeAndSettle(view, to: CGSize(width: width, height: height), token: token, measurement: &measurement)
                    let next = max(dedicated ? 1 : originalFrame.height, try geometry(measurement).height)
                    if abs(next - height) < 1 { settled = true; break }
                    height = next
                }
                guard settled else { throw SlopPackageError.invalid("Export layout keeps changing with viewport height; use normal flow in Export.svelte") }
                rect = CGRect(x: 0, y: 0, width: width, height: height)
            }
            let scale: CGFloat = output == .exportPNG || (isPreview && dedicated) ? 2 : 1
            try validateSize(width: width, height: height, output: output, scale: scale)
            let data: Data
            switch output {
            case .previewPNG, .exportPNG:
                let configuration = WKSnapshotConfiguration()
                configuration.rect = rect
                configuration.snapshotWidth = NSNumber(value: Double(width * scale))
                if dedicated { WebViewBackground.set(false, on: view) }
                let image = try await view.takeSnapshot(configuration: configuration)
                // The silhouette describes the window, so it clips only a capture of the
                // window. Dedicated views and longer full-length exports stay unmasked, like PDF.
                let windowSized = abs(rect.width - originalFrame.width) < 0.5 && abs(rect.height - originalFrame.height) < 0.5
                data = dedicated || !windowSized
                    ? try SlopPreviewImage.png(from: image)
                    : try SlopPreviewImage.png(from: image, package: session.package, scale: scale)
            case .pdf:
                let configuration = WKPDFConfiguration()
                configuration.rect = rect
                configuration.allowTransparentBackground = false
                data = try continuousPDF(try await view.pdf(configuration: configuration), size: rect.size)
            }
            return data
        }
    }

    /// WebKit splits captures taller than 14,400 points. Recompose its vector
    /// pages onto one continuous canvas without rasterizing text or artwork.
    private static func continuousPDF(_ data: Data, size: CGSize) throws -> Data {
        guard let provider = CGDataProvider(data: data as CFData), let source = CGPDFDocument(provider) else {
            throw SlopPackageError.invalid("Could not read captured PDF")
        }
        guard source.numberOfPages > 1 else { return data }
        let output = NSMutableData()
        // Keep page coordinates within Acrobat's supported range. Uniform vector
        // scaling preserves the complete document without an oversized MediaBox.
        let scale = min(1, 14_400 / max(size.width, size.height))
        var bounds = CGRect(x: 0, y: 0, width: size.width * scale, height: size.height * scale)
        guard bounds.width >= 3, bounds.height >= 3 else { throw SlopPackageError.invalid("Document aspect ratio exceeds single-page PDF limits") }
        guard let consumer = CGDataConsumer(data: output), let context = CGContext(consumer: consumer, mediaBox: &bounds, nil) else {
            throw SlopPackageError.invalid("Could not create continuous PDF")
        }
        context.beginPDFPage(nil)
        context.scaleBy(x: scale, y: scale)
        var top = size.height
        for number in 1...source.numberOfPages {
            guard let page = source.page(at: number) else { throw SlopPackageError.invalid("Missing captured PDF page") }
            let box = page.getBoxRect(.mediaBox)
            top -= box.height
            context.saveGState()
            context.translateBy(x: -box.minX, y: top - box.minY)
            context.drawPDFPage(page)
            context.restoreGState()
        }
        context.endPDFPage()
        context.closePDF()
        return output as Data
    }

    private static func captureFailure(_ error: Error) -> Error {
        if let message = (error as NSError).userInfo["WKJavaScriptExceptionMessage"] as? String {
            return SlopPackageError.invalid("Capture failed: \(message)")
        }
        return error
    }

    private static func validateSize(width: CGFloat, height: CGFloat, output: CaptureOutput, scale: CGFloat) throws {
        guard width.isFinite, height.isFinite, width > 0, height > 0 else { throw SlopPackageError.invalid("Invalid capture dimensions") }
        // PDF is vector output: a raster pixel budget would reject valid long documents.
        guard output != .pdf else { return }
        guard width * scale <= 16_384, height * scale <= 16_384, width * height * scale * scale <= 24_000_000 else {
            throw SlopPackageError.invalid("PNG exceeds 16384 pixels per side or 24 megapixels at \(Int(scale))×; export as PDF for longer documents")
        }
    }
    private static func geometry(_ value: [String: Any]) throws -> CGRect {
        guard let width = value["width"] as? NSNumber, let height = value["height"] as? NSNumber else { throw SlopPackageError.invalid("Could not measure capture content") }
        return CGRect(x: (value["x"] as? NSNumber)?.doubleValue ?? 0, y: (value["y"] as? NSNumber)?.doubleValue ?? 0, width: width.doubleValue, height: height.doubleValue)
    }
    private static func begin(_ view: WKWebView, token: String, mode: String) async throws -> [String: Any] {
        guard let value = try await view.callAsyncJavaScript("return await window.__hitslopCapture.begin(token, mode)", arguments: ["token": token, "mode": mode], in: nil, contentWorld: .page) as? [String: Any] else { throw SlopPackageError.invalid("Could not prepare capture") }
        return value
    }
    /// `begin` has already settled at the current size; only a resize needs another settle,
    /// which returns the settled measurement.
    private static func resizeAndSettle(_ view: WKWebView, to size: CGSize, token: String, measurement: inout [String: Any]) async throws {
        guard view.frame.size != size else { return }
        view.frame.size = size
        guard let value = try await view.callAsyncJavaScript("return await window.__hitslopCapture.settle(token)", arguments: ["token": token], in: nil, contentWorld: .page) as? [String: Any] else { throw SlopPackageError.invalid("Could not measure capture") }
        measurement = value
    }
    /// A user can resize the native window while an asynchronous capture is running.
    /// Restore the editor into today's container, not the frame from capture start.
    private static func restoreFrame(_ view: WKWebView, original: CGRect) {
        if let window = view.window, window.contentView !== view, let container = view.superview {
            view.frame = container.bounds
        } else {
            view.frame = original
        }
    }

    private static func restore(_ view: WKWebView, token: String) async throws {
        _ = try await view.callAsyncJavaScript("await window.__hitslopCapture.restore(token)", arguments: ["token": token], in: nil, contentWorld: .page)
    }

}
