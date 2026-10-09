import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument
import WebKit

@MainActor public enum SlopRenderer {
  private enum CaptureOutput { case previewPNG, exportPNG, pdf }

  public static func previewPNGData(url: URL) async throws -> Data {
    try await withRenderSession(url: url) { try await capture(session: $0, output: .previewPNG) }
  }
  static func previewPNGData(session: DocumentSession) async throws -> Data {
    try await withSavedRenderer(session) { try await capture(session: $0, output: .previewPNG) }
  }
  public static func exportPNGData(session: DocumentSession) async throws -> Data {
    try await withSavedRenderer(session) { try await capture(session: $0, output: .exportPNG) }
  }
  public static func exportPDFData(session: DocumentSession) async throws -> Data {
    try await withSavedRenderer(session) { try await capture(session: $0, output: .pdf) }
  }

  /// Renders artwork from a source already acquired by a copy operation.
  static func artwork(url: URL, telemetry: SlopTelemetry) async -> SlopRenderedArtwork? {
    do {
      return try await withRenderSession(url: url) { await artwork(session: $0, telemetry: telemetry) }
    } catch {
      if !SlopFailureContext.isCancellation(error) { telemetry.send(.failed(.artwork, .init(reason: .preview))) }
      return nil
    }
  }

  /// The artwork a closing window writes into its document: its preview and, when the
  /// app draws one, its icon. A capture that fails is reported and left out.
  public static func artwork(session: DocumentSession, telemetry: SlopTelemetry, trace: SlopCloseTrace? = nil) async
    -> SlopRenderedArtwork
  {
    do {
      return try await withSavedRenderer(session, trace: trace) { renderer in
        var preview: Data?
        var icon: Data?
        do {
          let interval = trace?.begin("Preview capture")
          defer { trace?.end("Preview capture", interval) }
          preview = try await capture(session: renderer, output: .previewPNG)
        } catch {
          if !SlopFailureContext.isCancellation(error) { telemetry.send(.failed(.artwork, .init(reason: .preview))) }
        }
        do {
          let interval = trace?.begin("Icon capture")
          defer { trace?.end("Icon capture", interval) }
          icon = try await captureIcon(session: renderer)
        } catch {
          if !SlopFailureContext.isCancellation(error) { telemetry.send(.failed(.artwork, .init(reason: .icon))) }
        }
        return SlopRenderedArtwork(preview: preview, icon: icon)
      }
    } catch {
      if !SlopFailureContext.isCancellation(error) { telemetry.send(.failed(.artwork, .init(reason: .preview))) }
      return SlopRenderedArtwork(preview: nil, icon: nil)
    }
  }

  private static func withSavedRenderer<T>(
    _ session: DocumentSession,
    trace: SlopCloseTrace? = nil,
    _ capture: @MainActor (DocumentSession) async throws -> T
  ) async throws -> T {
    if session.isSnapshot { return try await capture(session) }
    return try await session.withCaptureSnapshot(trace: trace) { source in
      try await withRenderSession(url: source, trace: trace, capture)
    }
  }

  /// Background renders read the saved document as a snapshot: they take no ownership
  /// and never write to the file.
  static func withRenderSession<T>(
    url: URL,
    trace: SlopCloseTrace? = nil,
    _ capture: @MainActor (DocumentSession) async throws -> T
  ) async throws -> T {
    var startup = trace?.begin("Renderer startup")
    defer { trace?.end("Renderer startup", startup) }
    let session = try await DocumentSession.open(url: url, storage: .snapshot)
    let window = hiddenWindow(session)
    let result: Result<T, Error>
    do {
      session.load()
      try await session.waitUntilReady()
      trace?.end("Renderer startup", startup)
      startup = nil
      try Task.checkCancellation()
      result = .success(try await capture(session))
    } catch { result = .failure(error) }
    window.contentView = nil
    try await session.close()
    return try result.get()
  }

  private static func hiddenWindow(_ session: DocumentSession) -> NSWindow {
    let window = NSWindow(
      contentRect: session.webView.frame, styleMask: [.borderless], backing: .buffered, defer: false)
    window.contentView = session.webView
    window.orderOut(nil)
    return window
  }

  public static func iconPNGData(url: URL) async throws -> Data? {
    try await withRenderSession(url: url) { session in
      try await iconPNGData(session: session)
    }
  }

  public static func iconPNGData(session: DocumentSession) async throws -> Data? {
    try await withSavedRenderer(session) { try await captureIcon(session: $0) }
  }

  private static func captureIcon(session: DocumentSession) async throws -> Data? {
    try await withCapture(session) { view, token, originalFrame in
      view.frame.size = CGSize(width: max(512, originalFrame.width), height: max(512, originalFrame.height))
      let value = try await begin(view, token: token, mode: .icon)
      guard value.dedicated else { return nil }
      let rect = try geometry(value)
      guard rect.width > 0, abs(rect.width - rect.height) < 0.5,
        rect.minX >= -0.5, rect.minY >= -0.5,
        rect.maxX <= view.bounds.width + 0.5, rect.maxY <= view.bounds.height + 0.5
      else {
        throw SlopFailure("icon target must be a visible square inside the capture viewport")
      }
      WebViewBackground.set(false, on: view)
      let configuration = WKSnapshotConfiguration()
      configuration.rect = rect
      configuration.snapshotWidth = 512
      return try SlopPreviewImage.png(from: try await view.takeSnapshot(configuration: configuration))
    }
  }

  /// Capture only a disposable read-only page. Reset the render viewport between
  /// preview and icon; no editor focus, frame or input state is involved.
  private static func withCapture<T>(
    _ session: DocumentSession,
    _ body: @MainActor (_ view: WKWebView, _ token: String, _ originalFrame: CGRect) async throws -> T
  ) async throws -> T {
    try await session.withCapture {
      try Task.checkCancellation()
      let view = session.webView
      let token = UUID().uuidString
      let frame = view.frame
      let background = WebViewBackground.get(view)
      defer {
        view.frame = frame
        WebViewBackground.set(background, on: view)
      }
      do {
        let result = try await body(view, token, frame)
        try await restore(view, token: token)
        try Task.checkCancellation()
        return result
      } catch {
        try? await restore(view, token: token)
        throw captureFailure(error)
      }
    }
  }

  private static func capture(session: DocumentSession, output: CaptureOutput) async throws -> Data {
    let isPreview = output == .previewPNG
    return try await withCapture(session) { view, token, originalFrame in
      var measurement = try await begin(view, token: token, mode: isPreview ? .preview : .export)
      let dedicated = measurement.dedicated
      var width = dedicated && !isPreview ? max(1, try geometry(measurement).width) : originalFrame.width
      var height =
        isPreview ? originalFrame.height : max(dedicated ? 1 : originalFrame.height, try geometry(measurement).height)
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
          try validateSize(
            width: max(min(box.width, viewWidth), 1), height: max(min(box.height, viewHeight), 1), output: output,
            scale: 2)
          try await resizeAndSettle(
            view, to: CGSize(width: viewWidth, height: viewHeight), token: token, measurement: &measurement)
          let next = try geometry(measurement)
          let bounds = CGRect(x: 0, y: 0, width: viewWidth, height: viewHeight)
          let visible = next.intersection(bounds)
          // Content continuing below a height-limited viewport is expected.
          if next.minX >= -0.5, next.minY >= -0.5,
            next.maxX <= viewWidth + 48, next.maxY <= viewHeight + 48 || viewHeight >= heightLimit,
            visible.width >= 8, visible.height >= 8
          {
            settled = true
            box = visible
            break
          }
          box = next
        }
        guard settled else {
          throw SlopFailure("Preview export layout keeps changing with viewport size; use normal flow in Export.svelte")
        }
        width = max(box.width, 1)
        height = max(min(box.height, previewHeight(box)), 1)
        rect = CGRect(x: box.minX, y: box.minY, width: width, height: height)
      } else if output == .exportPNG || (output == .pdf && dedicated) {
        var settled = false
        for _ in 0..<4 {
          try validateSize(width: width, height: height, output: output, scale: output == .pdf ? 1 : 2)
          try await resizeAndSettle(
            view, to: CGSize(width: width, height: output == .pdf ? originalFrame.height : height),
            token: token, measurement: &measurement)
          let box = try geometry(measurement)
          let nextWidth = dedicated ? max(1, box.width) : originalFrame.width
          let nextHeight = max(dedicated ? 1 : originalFrame.height, box.height)
          if abs(nextHeight - height) < 1 && abs(nextWidth - width) < 1 {
            settled = true
            rect = CGRect(x: dedicated ? box.minX : 0, y: dedicated ? box.minY : 0, width: width, height: height)
            break
          }
          width = nextWidth
          height = nextHeight
        }
        guard settled else {
          throw SlopFailure("Export layout keeps changing with viewport size; use normal flow in Export.svelte")
        }
      }
      // Large standard windows that fit at 1× must still have a preview.
      // Explicit PNG exports retain the documented 2× output and its size limit.
      let doubleSizeFits =
        width * 2 <= CGFloat(Limits.imageSide)
        && height * 2 <= CGFloat(Limits.imageSide) && width * height * 4 <= CGFloat(Limits.imagePixels)
      let scale: CGFloat = output == .exportPNG || (isPreview && (dedicated || doubleSizeFits)) ? 2 : 1
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
        data =
          dedicated || !windowSized
          ? try SlopPreviewImage.png(from: image)
          : try SlopPreviewImage.png(from: image, file: session.file, scale: scale)
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
      throw SlopFailure("Could not read captured PDF")
    }
    guard source.numberOfPages > 1 else { return data }
    let output = NSMutableData()
    // Keep page coordinates within Acrobat's supported range. Uniform vector
    // scaling preserves the complete document without an oversized MediaBox.
    let scale = min(1, 14_400 / max(size.width, size.height))
    var bounds = CGRect(x: 0, y: 0, width: size.width * scale, height: size.height * scale)
    guard bounds.width >= 3, bounds.height >= 3 else {
      throw SlopFailure("Document aspect ratio exceeds single-page PDF limits")
    }
    guard let consumer = CGDataConsumer(data: output),
      let context = CGContext(consumer: consumer, mediaBox: &bounds, nil)
    else {
      throw SlopFailure("Could not create continuous PDF")
    }
    context.beginPDFPage(nil)
    context.scaleBy(x: scale, y: scale)
    var top = size.height
    for number in 1...source.numberOfPages {
      guard let page = source.page(at: number) else { throw SlopFailure("Missing captured PDF page") }
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
      return SlopFailure("Capture failed: \(message)")
    }
    return error
  }

  private static func validateSize(width: CGFloat, height: CGFloat, output: CaptureOutput, scale: CGFloat) throws {
    guard width.isFinite, height.isFinite, width > 0, height > 0 else {
      throw SlopFailure("Invalid capture dimensions")
    }
    // PDF is vector output: a raster pixel budget would reject valid long documents.
    guard output != .pdf else { return }
    guard width * scale <= CGFloat(Limits.imageSide), height * scale <= CGFloat(Limits.imageSide),
      width * height * scale * scale <= CGFloat(Limits.imagePixels)
    else {
      throw SlopFailure(
        "PNG exceeds 16384 pixels per side or 24 megapixels at \(Int(scale))×; export as PDF for longer documents")
    }
  }
  private static func geometry(_ value: HostCaptureResult) throws -> CGRect {
    guard value.width.isFinite, value.height.isFinite, value.x.isFinite, value.y.isFinite else {
      throw SlopFailure("Could not measure capture content")
    }
    return CGRect(x: value.x, y: value.y, width: value.width, height: value.height)
  }
  private static func begin(_ view: WKWebView, token: String, mode: CaptureMode) async throws
    -> HostCaptureResult
  {
    guard let value = try await view.callHost(.captureBegin(token: token, mode: mode)) as? String else {
      throw SlopFailure("Could not prepare capture")
    }
    return try decodeHostCaptureResult(json: value)
  }
  private static func resizeAndSettle(
    _ view: WKWebView, to size: CGSize, token: String, measurement: inout HostCaptureResult
  ) async throws {
    guard view.frame.size != size else { return }
    view.frame.size = size
    guard let value = try await view.callHost(.captureSettle(token: token)) as? String else {
      throw SlopFailure("Could not measure capture")
    }
    measurement = try decodeHostCaptureResult(json: value)
  }
  private static func restore(_ view: WKWebView, token: String) async throws {
    _ = try await view.callHost(.captureRestore(token: token))
  }
}
