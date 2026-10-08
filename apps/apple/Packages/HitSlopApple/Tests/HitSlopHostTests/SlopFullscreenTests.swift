import AppKit
import HitSlopCore
import HitSlopTestSupport
import Testing
import WebKit

@testable import HitSlopHost

@MainActor private func fullscreenFixture(_ kind: String, enabled: Bool = true) throws -> URL {
  var window: [String: Any] = ["width": 320, "height": 240, "fullscreenable": enabled]
  if kind == "shape" {
    window["shape"] = "50%"
    window["lockAspect"] = true
  }
  if kind == "fixed" { window["resizable"] = false }
  let stage = try Fixtures.minimalStage(fields: ["window": window])
  try Fixtures.writeApp(
    """
    export default { mount(ctx, target) {
      const surface = document.createElement('main');
      surface.style.cssText = 'position:fixed;inset:0;background:#168c99;color:white;display:grid;place-content:center;text-align:center;font:600 20px system-ui';
      surface.textContent = 'Fullscreen test · \(kind)';
      target.append(surface);
      return { unmount() { surface.remove(); } };
    } };
    """, to: stage)
  if kind == "skin" {
    let png = try Fixtures.png(width: 320, height: 240) { rect in
      NSColor.systemOrange.setFill()
      NSBezierPath(ovalIn: rect).fill()
    }
    try png.write(to: stage.appendingPathComponent("skin.png"))
    try Fixtures.addSkin(stage, path: "skin.png")
    try Fixtures.updateApp(stage) { app in
      var spec = app["window"] as! [String: Any]
      spec["fullscreenable"] = enabled
      app["window"] = spec
    }
  }
  return try Fixtures.document(stage: stage, at: Fixtures.folder().appendingPathComponent("fullscreen.slop"))
}

extension HostTests {
  @Test @MainActor func fullscreenCapabilityAndLayoutReachTheNativeHost() async throws {
    for kind in ["standard", "fixed", "shape", "skin"] {
      let url = try fullscreenFixture(kind)
      defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
      let controller = try await SlopDocumentWindowController.open(url: url)
      let window = try #require(controller.window)
      #expect(controller.session.file.isFullscreenable)
      #expect(controller.session.file.fitsFullscreen == (kind != "standard"))
      #expect(window.collectionBehavior.contains(.fullScreenPrimary))
      let composition = try #require(controller.documentComposition)
      let view = controller.session.webView
      let surface = SlopFullscreenSurface(
        composition: composition, authoredSize: NSSize(width: 320, height: 240), fit: kind != "standard")
      surface.frame = NSRect(x: 0, y: 0, width: 1200, height: 600)
      surface.layout()
      #expect(view.superview === composition)
      #expect(view.frame == composition.bounds)
      let hit = composition.hitTest(NSPoint(x: composition.frame.midX, y: composition.frame.midY))
      #expect(hit === view || hit?.isDescendant(of: view) == true)
      if kind == "shape" || kind == "skin" {
        #expect(composition.hitTest(NSPoint(x: composition.frame.minX + 1, y: composition.frame.minY + 1)) == nil)
      }
      if kind == "standard" {
        #expect(composition.frame == surface.bounds)
        #expect(composition.bounds.size == NSSize(width: 1200, height: 600))
      } else {
        #expect(composition.frame == NSRect(x: 200, y: 0, width: 800, height: 600))
        #expect(composition.bounds.size == NSSize(width: 800, height: 600))
        #expect(view.pageZoom == 2.5)
        // AppKit remains at display resolution; WebKit maps input into authored CSS coordinates.
        #expect(composition.convert(NSPoint(x: 600, y: 300), from: surface) == NSPoint(x: 400, y: 300))
      }
      try await controller.session.close()
      controller.close()
    }
    let url = try fullscreenFixture("standard", enabled: false)
    defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
    let controller = try await SlopDocumentWindowController.open(url: url)
    #expect(!controller.toolbarControls.fullscreenable)
    #expect(!controller.canToggleFullscreen)
    #expect(controller.window?.collectionBehavior.contains(.fullScreenNone) == true)
    try await controller.session.close()
    controller.close()
  }

  // Native Spaces transitions need an interactive desktop. Run deliberately, not
  // concurrently with unrelated tests that activate their own document windows.
  @Test(.enabled(if: ProcessInfo.processInfo.environment["HITSLOP_FULLSCREEN"] == "1"))
  @MainActor func nativeFullscreenKeepsThePageAndRestoresTheDesktop() async throws {
    NSApplication.shared.setActivationPolicy(.regular)
    for kind in ["standard", "fixed", "shape", "skin"] {
      let url = try fullscreenFixture(kind)
      defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
      let controller = try await SlopDocumentWindowController.open(url: url)
      do {
        controller.showWindow(nil)
        NSApp.activate()
        await eventually(timeout: .seconds(15)) { controller.isContentReady }
        try #require(controller.isContentReady)
        let window = try #require(controller.window)
        window.makeKeyAndOrderFront(nil)
        controller.togglePin()
        let before = window.frame
        let style = window.styleMask
        let aspect = window.contentAspectRatio
        let level = window.level
        let view = controller.session.webView
        _ = try await view.evaluateJavaScript(
          "globalThis.fullscreenProbe = 42; globalThis.fullscreenTicks = 0; setInterval(() => fullscreenTicks++, 40)")
        controller.toggleFullscreen()
        await eventually(timeout: .seconds(10)) { controller.isFullscreen && !controller.fullscreenTransition }
        try #require(controller.isFullscreen && !controller.fullscreenTransition)
        #expect(!controller.canPin)
        #expect(controller.session.webView === view)
        #expect(try await view.evaluateJavaScript("globalThis.fullscreenProbe") as? Int == 42)
        window.contentView?.layoutSubtreeIfNeeded()
        let content = try #require(window.contentView)
        #expect(abs(content.frame.width - (window.screen?.frame.width ?? 0)) < 2)
        let screen = try #require(window.screen)
        // AppKit reserves the menu bar when the person's fullscreen preference
        // keeps it visible. The surface must fill the window's entire viewport.
        #expect(window.frame.height >= screen.visibleFrame.height - 1)
        #expect(window.frame.height <= screen.frame.height + 1)
        #expect(abs(content.frame.height - window.frame.height) < 1)
        let viewport = try #require(try await view.evaluateJavaScript("[innerWidth, innerHeight]") as? [Double])
        let expected = kind == "standard" ? content.bounds.size : NSSize(width: 320, height: 240)
        #expect(viewport.count == 2)
        #expect(abs(viewport[0] - Double(expected.width)) < 1)
        #expect(abs(viewport[1] - Double(expected.height)) < 1)
        let composition = try #require(controller.documentComposition)
        #expect(view.bounds.size == composition.frame.size)
        // Check painted content as well as layout: a live JS page and correct
        // viewport dimensions alone do not prove WebKit painted the whole view.
        let snapshot = try await view.takeSnapshot(configuration: nil)
        let pixels = try #require(snapshot.tiffRepresentation)
        let bitmap = try #require(NSBitmapImageRep(data: pixels))
        for x in [bitmap.pixelsWide / 5, bitmap.pixelsWide * 4 / 5] {
          for y in [bitmap.pixelsHigh / 5, bitmap.pixelsHigh * 4 / 5] {
            let color = try #require(bitmap.colorAt(x: x, y: y)?.usingColorSpace(.sRGB))
            #expect(color.redComponent < 0.15)
            #expect(color.greenComponent > 0.45 && color.greenComponent < 0.65)
            #expect(color.blueComponent > 0.5 && color.blueComponent < 0.7)
          }
        }
        controller.toolbar.refresh(
          point: NSPoint(x: window.frame.midX, y: window.frame.maxY - 2), front: window.windowNumber)
        #expect(controller.toolbar.isVisible)
        #expect(controller.toolbar.panel?.isOnActiveSpace == true)
        controller.toggleFullscreen()
        await eventually(timeout: .seconds(10)) { !controller.isFullscreen && !controller.fullscreenTransition }
        try #require(!controller.isFullscreen && !controller.fullscreenTransition)
        #expect(controller.isPinned)
        #expect(window.frame == before)
        #expect(window.styleMask == style)
        #expect(window.contentAspectRatio == aspect)
        #expect(window.level == level)
        #expect(view === controller.session.webView)
        #expect(try await view.evaluateJavaScript("globalThis.fullscreenProbe") as? Int == 42)
        #expect((try await view.evaluateJavaScript("globalThis.fullscreenTicks") as? Int ?? 0) > 0)
        #expect(view.frame == controller.documentComposition?.bounds)
        #expect(view.pageZoom == 1)
        if kind == "standard" {
          // A renderer replacement must attach inside the fullscreen composition,
          // and its failure overlay must survive exiting the fullscreen surface.
          controller.toggleFullscreen()
          await eventually(timeout: .seconds(10)) { controller.isFullscreen && !controller.fullscreenTransition }
          try #require(controller.isFullscreen && !controller.fullscreenTransition)
          let pid = try #require(view.value(forKey: "_webProcessIdentifier") as? Int32)
          try #require(pid > 0)
          try #require(Darwin.kill(pid, SIGKILL) == 0)
          await eventually(timeout: .seconds(5)) { controller.session.rendererDead && controller.failedOverlay != nil }
          try #require(controller.session.rendererDead)
          #expect(controller.canToggleFullscreen)
          try await controller.session.reopenSavedDocument()
          await controller.waitForPresentation()
          #expect(controller.session.webView !== view)
          #expect(controller.isFullscreen)
          #expect(controller.isContentReady)
          #expect(controller.failedOverlay == nil)
          #expect(controller.session.webView.superview === controller.documentComposition)
          controller.toggleFullscreen()
          await eventually(timeout: .seconds(10)) { !controller.isFullscreen && !controller.fullscreenTransition }
          #expect(window.frame == before)
        }
        try await controller.session.close()
        controller.close()
      } catch {
        try? await controller.session.close()
        controller.close()
        throw error
      }
    }
  }
}
