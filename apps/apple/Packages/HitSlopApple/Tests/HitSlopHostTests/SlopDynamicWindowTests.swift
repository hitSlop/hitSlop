import AppKit
import Testing

@testable import HitSlopHost

@Test func dynamicWindowResizeKeepsTheTopLeftAnchor() {
  let current = NSRect(x: 200, y: 300, width: 275, height: 438)
  let frame = dynamicSlopWindowFrame(
    current: current,
    requested: NSSize(width: 725, height: 438),
    visible: NSRect(x: 0, y: 0, width: 1440, height: 900)
  )

  #expect(frame.minX == current.minX)
  #expect(frame.maxY == current.maxY)
  #expect(frame.size == NSSize(width: 725, height: 438))
}

@Test func dynamicWindowResizeFitsTheVisibleWorkArea() {
  let visible = NSRect(x: 0, y: 40, width: 600, height: 460)
  let frame = dynamicSlopWindowFrame(
    current: NSRect(x: 400, y: 300, width: 275, height: 438),
    requested: NSSize(width: 725, height: 600),
    visible: visible
  )

  #expect(frame == visible)
}
