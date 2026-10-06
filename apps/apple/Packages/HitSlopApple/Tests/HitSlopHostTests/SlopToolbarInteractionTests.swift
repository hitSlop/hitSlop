import AppKit
import HitSlopDocument
import HitSlopTestSupport
import PDFKit
@testable import HitSlopHost
import Testing


@Test func toolbarHoverRecoversAndAllowsGapCrossing() {
  var state = SlopToolbarVisibility()
  let result12 = !state.shouldShow(inside: false, interacting: false, visible: false, now: 0)
  #expect(result12)
  let result13 = state.shouldShow(inside: true, interacting: false, visible: false, now: 1)
  #expect(result13)
  let result14 = state.shouldShow(inside: false, interacting: false, visible: true, now: 2)
  #expect(result14)
  let result15 = state.shouldShow(inside: false, interacting: false, visible: true, now: 2.7)
  #expect(result15)
  let result16 = !state.shouldShow(inside: false, interacting: false, visible: true, now: 2.9)
  #expect(result16)
  let result17 = state.shouldShow(inside: true, interacting: false, visible: false, now: 3)
  #expect(result17)
  #expect(state.outsideSince == nil)
}

@Test func toolbarInteractionResetsHideDeadline() {
  var state = SlopToolbarVisibility()
  let result23 = state.shouldShow(inside: false, interacting: false, visible: true, now: 0)
  #expect(result23)
  let result24 = state.shouldShow(inside: false, interacting: true, visible: true, now: 10)
  #expect(result24)
  let result25 = state.shouldShow(inside: false, interacting: false, visible: true, now: 11)
  #expect(result25)
  let result26 = state.shouldShow(inside: false, interacting: false, visible: true, now: 11.7)
  #expect(result26)
  let result27 = !state.shouldShow(inside: false, interacting: false, visible: true, now: 11.9)
  #expect(result27)
}

extension HostTests {
  // Clicks pass through a transparent document's empty pixels, so the window server reports
  // the window behind it there. The document's shape still decides hover unless another
  // window covers the point.
  @Test @MainActor func toolbarFollowsShapeOverTransparentPixels() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    SlopToolbarPointerSampler.shared.remove(controller.toolbar)
    await controller.waitForPresentation()
    do {
      controller.showWindow(nil)
      let window = try #require(controller.window)
      let inside = NSPoint(x: window.frame.midX, y: window.frame.midY)
      let toolbarVisible = { NSApp.windows.contains { $0 !== window && controller.owns($0) && $0.isVisible } }
      let behind = window.windowNumber + 1_000, cover = window.windowNumber + 2_000
      controller.toolbar.refresh(point: inside, front: cover, below: behind, now: 1)
      #expect(!toolbarVisible())
      controller.toolbar.refresh(point: inside, front: behind, below: behind, now: 2)
      #expect(toolbarVisible())
      try await controller.session.close()
    } catch {
      try? await controller.session.close()
      controller.window?.close()
      throw error
    }
    controller.window?.close()
  }

  // A toolbar first shown while unpinned must still float above other apps' windows
  // after pinning: a window ordered front over it (a click in another app) must not bury it.
  @Test @MainActor func pinnedToolbarStaysAboveOtherWindows() async throws {
    _ = NSApplication.shared
    let root = try contractFixture()
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    SlopToolbarPointerSampler.shared.remove(controller.toolbar)
    await controller.waitForPresentation()
    var other: NSWindow?
    do {
      controller.showWindow(nil)
      let window = try #require(controller.window)
      controller.toolbar.refresh(point: NSPoint(x: window.frame.midX, y: window.frame.midY),
                                     front: window.windowNumber, now: 1)
      let toolbar = try #require(NSApp.windows.first { $0 !== window && controller.owns($0) })
      controller.setPinned(true)
      let cover = NSWindow(contentRect: toolbar.frame.union(window.frame), styleMask: [.borderless],
                           backing: .buffered, defer: false)
      cover.isReleasedWhenClosed = false
      other = cover
      cover.orderFrontRegardless()
      let order = (NSWindow.windowNumbers(options: []) ?? []).map(\.intValue)
      let rank = { (window: NSWindow) in order.firstIndex(of: window.windowNumber) ?? .max }
      #expect(toolbar.isVisible)
      #expect(rank(toolbar) < rank(cover))
      #expect(rank(toolbar) < rank(window))
      other?.close()
      try await controller.session.close()
    } catch {
      other?.close()
      try? await controller.session.close()
      controller.window?.close()
      throw error
    }
    controller.window?.close()
  }

  // A guest must not retain visible/focusable controls after native chrome hides.
  // Existing deadline tests do not exercise delivery into a real WKWebView.
  @Test @MainActor func guestControlsFollowNativeToolbar() async throws {
    _ = NSApplication.shared
    let root = try contractFixture { stage in
      // Wrap the probe app with an authored control that follows the native toolbar.
      let assets = stage.appendingPathComponent("assets")
      try FileManager.default.moveItem(at: assets.appendingPathComponent("app.js"), to: assets.appendingPathComponent("probe.js"))
      try Data("""
        import probe from "./probe.js";
        export default { mount(ctx, target) {
          const style = document.createElement("style");
          style.textContent = '#hover-control { visibility: hidden; pointer-events: none; } html[data-slop-controls="visible"] #hover-control { visibility: visible; pointer-events: auto; }';
          document.head.append(style);
          const button = document.createElement("button");
          button.id = "hover-control"; button.dataset.slopExport = "hide"; button.textContent = "Hover action";
          target.append(button);
          return probe.mount(ctx, target);
        } };
        """.utf8).write(to: assets.appendingPathComponent("app.js"))
    }
    defer { try? FileManager.default.removeItem(at: root) }
    let controller = try await SlopDocumentWindowController.open(url: root)
    SlopToolbarPointerSampler.shared.remove(controller.toolbar)
    await controller.waitForPresentation()
    let view = controller.session.webView
    do {
      // The host must initialize the signal before any pointer interaction.
      try #require(try await view.evaluateJavaScript("document.documentElement.getAttribute('data-slop-controls')") as? String == "hidden")
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "hidden")
      controller.showWindow(nil)
      let window = try #require(controller.window)
      let inside = NSPoint(x: window.frame.midX, y: window.frame.midY)
      controller.toolbar.refresh(point: inside, front: window.windowNumber, now: 1)
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "visible")
      _ = try await view.evaluateJavaScript("document.querySelector('#hover-control').focus(); true")
      #expect(try await view.evaluateJavaScript("document.activeElement.id") as? String == "hover-control")

      let toolbar = try #require(NSApp.windows.first { $0 !== window && controller.owns($0) })
      controller.toolbar.refresh(point: NSPoint(x: toolbar.frame.midX, y: toolbar.frame.midY),
                                     front: toolbar.windowNumber, now: 2)
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "visible")
      let outside = NSPoint(x: window.frame.maxX + 500, y: window.frame.maxY + 500)
      controller.toolbar.refresh(point: outside, front: 0, now: 3)
      #expect(toolbar.isVisible)
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "visible")
      controller.toolbar.refresh(point: outside, front: 0, now: 3.9)
      #expect(!toolbar.isVisible)
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "hidden")
      #expect(try await view.evaluateJavaScript("document.querySelector('#draft').focus(); document.querySelector('#hover-control').focus(); document.activeElement.id") as? String == "draft")

      controller.toolbar.refresh(point: inside, front: window.windowNumber)
      _ = try await view.evaluateJavaScript("document.querySelector('#hover-control').disabled = true; true")
      // Miniaturization immediately hides chrome, including a busy guest control.
      controller.windowWillMiniaturize(Notification(name: NSWindow.willMiniaturizeNotification, object: window))
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "hidden")
      controller.toolbar.refresh(point: inside, front: window.windowNumber)
      window.orderOut(nil)
      controller.toolbar.refresh()
      #expect(try await view.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "hidden")
      let pdf = try await SlopRenderer.exportPDFData(session: controller.session)
      #expect(PDFDocument(data: pdf)?.string?.contains("Hover action") == false)
      #expect(try await view.evaluateJavaScript("document.documentElement.getAttribute('data-slop-controls')") as? String == "hidden")

      let pid = try #require(view.value(forKey: "_webProcessIdentifier") as? Int32)
      try #require(pid > 0)
      try #require(Darwin.kill(pid, SIGKILL) == 0)
      await eventually(timeout: .seconds(5)) { controller.session.rendererDead }
      try #require(controller.session.rendererDead)
      try await controller.session.reopenSavedDocument()
      // The replacement page reloads like any recovery; interact once it is presented.
      await controller.waitForPresentation()
      #expect(try await controller.session.webView.evaluateJavaScript("document.documentElement.getAttribute('data-slop-controls')") as? String == "hidden")
      window.orderFront(nil)
      controller.toolbar.refresh(point: inside, front: window.windowNumber)
      #expect(try await controller.session.webView.evaluateJavaScript("getComputedStyle(document.querySelector('#hover-control')).visibility") as? String == "visible")
      try await controller.session.close()
    } catch {
      try? await controller.session.close()
      controller.window?.close()
      throw error
    }
    controller.window?.close()
  }
}
