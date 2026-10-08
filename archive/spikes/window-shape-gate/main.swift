// Two actual WindowServer windows in separate processes. No sendEvent oracle.
import AppKit
import QuartzCore

let receiver = CommandLine.arguments.contains("--receiver")
let manual = CommandLine.arguments.contains("--manual")
let prefix = CommandLine.arguments.last!
let result = URL(fileURLWithPath: prefix + (receiver ? ".receiver" : ".foreground"))
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let screen = NSScreen.main!
let size = NSSize(width: 320, height: 280)
let origin = NSPoint(x: screen.visibleFrame.midX - 160, y: screen.visibleFrame.midY - 140)
var hits = 0
var other: Process?
var resized = false
final class ProbeWindow: NSWindow { override var canBecomeKey: Bool { !receiver } }
let window = ProbeWindow(contentRect: NSRect(origin: receiver ? NSPoint(x: origin.x - 60, y: origin.y - 60) : origin,
    size: receiver ? NSSize(width: 540, height: 440) : size), styleMask: [.borderless], backing: .buffered, defer: false)
final class Probe: NSView {
  let ring: Bool
  init(ring: Bool) {
    self.ring = ring
    super.init(frame: .zero)
    wantsLayer = true
    layer!.backgroundColor = (ring ? NSColor.systemOrange : NSColor.systemBlue).cgColor
    if ring { layer!.mask = CAShapeLayer() }
  }
  required init?(coder: NSCoder) { nil }
  var silhouette: CGPath {
    let path = CGMutablePath()
    path.addRoundedRect(in: bounds.insetBy(dx: 8, dy: 8), cornerWidth: 30, cornerHeight: 30)
    path.addEllipse(in: CGRect(x: bounds.width * 0.20, y: bounds.height * 0.25, width: bounds.width * 0.5, height: bounds.height * 0.5))
    return path
  }
  override func layout() {
    super.layout()
    if let mask = layer?.mask as? CAShapeLayer {
      mask.frame = bounds; mask.path = silhouette; mask.fillRule = .evenOdd
    }
  }
  override func draw(_ dirtyRect: NSRect) {
    super.draw(dirtyRect)
    let label = ring ? "FOREGROUND \(hits)   R: resize · Q: quit" : "RECEIVER \(hits) — blue receives hole clicks"
    (label as NSString).draw(at: NSPoint(x: 18, y: bounds.height - 38), withAttributes: [
      .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .bold), .foregroundColor: NSColor.black])
  }
  override func hitTest(_ point: NSPoint) -> NSView? {
    !ring || silhouette.contains(point, using: .evenOdd) ? super.hitTest(point) : nil
  }
  override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
  override func mouseDown(with event: NSEvent) {
    hits += 1
    try? Data(String(hits).utf8).write(to: result, options: .atomic)
    let point = convert(event.locationInWindow, from: nil)
    let line = "\(Date().timeIntervalSince1970) \(ring ? "foreground" : "receiver") x=\(Int(point.x)) y=\(Int(point.y)) count=\(hits)\n"
    let events = URL(fileURLWithPath: result.path + ".events")
    if !FileManager.default.fileExists(atPath: events.path) { FileManager.default.createFile(atPath: events.path, contents: nil) }
    if let file = try? FileHandle(forWritingTo: events) { file.seekToEndOfFile(); file.write(Data(line.utf8)); try? file.close() }
    needsDisplay = true
  }
}
func count(_ suffix: String) -> Int {
  Int((try? String(contentsOfFile: prefix + suffix, encoding: .utf8)) ?? "0") ?? 0
}
func finish(_ code: Int32) -> Never {
  other?.terminate(); window.close(); exit(code)
}
if !receiver {
  if !manual && !CGPreflightPostEventAccess() {
    print("BLOCKED: event posting is not authorized. Use --manual for real mouse input; no result claimed.")
    exit(2)
  }
  for suffix in [".foreground", ".receiver"] {
    try? Data("0".utf8).write(to: URL(fileURLWithPath: prefix + suffix), options: .atomic)
    try? Data().write(to: URL(fileURLWithPath: prefix + suffix + ".events"), options: .atomic)
  }
  let child = Process()
  child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0])
  child.arguments = ["--receiver", prefix]
  try! child.run(); other = child
}
window.isOpaque = false
window.backgroundColor = .clear
window.hasShadow = true
window.isReleasedWhenClosed = false
window.contentView = Probe(ring: !receiver)
window.orderFrontRegardless()
func resize() {
  resized.toggle()
  window.setContentSize(resized ? NSSize(width: 400, height: 320) : size)
}
func click(_ local: CGPoint) {
  let point = CGPoint(x: window.frame.minX + local.x, y: screen.frame.maxY - window.frame.minY - local.y)
  for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left)!.post(tap: .cghidEventTap)
  }
}
if !receiver {
  DispatchQueue.main.asyncAfter(deadline: .now() + 1) { window.makeKeyAndOrderFront(nil); app.activate(ignoringOtherApps: true) }
  if manual {
    print("MANUAL: click the blue hole, orange rim, and blue outside. Press R to resize and repeat; Q quits. Counts are \(prefix).foreground/.receiver. No automatic pass is inferred.")
    fflush(stdout)
    _ = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
      if event.charactersIgnoringModifiers == "r" { resize(); return nil }
      if event.charactersIgnoringModifiers == "q" { finish(0) }
      return event
    }
    // Activation of the receiver must not hide the foreground for subsequent samples.
    // Restack only after a received click; this does not synthesize or redirect input.
    var receiverHits = 0
    Timer.scheduledTimer(withTimeInterval: 0.3, repeats: true) { _ in
      let next = count(".receiver")
      if next != receiverHits { receiverHits = next; window.makeKeyAndOrderFront(nil); app.activate(ignoringOtherApps: true) }
    }
    // Keep a diagnostic accidentally left open from living indefinitely.
    DispatchQueue.main.asyncAfter(deadline: .now() + 600) { finish(0) }
  } else {
    var valid = true
    let points: [(CGPoint, Bool)] = [(.init(x: 160, y: 140), false), (.init(x: 35, y: 140), true), (.init(x: -20, y: 140), false), (.init(x: 200, y: 160), false)]
    for (index, sample) in points.enumerated() {
      DispatchQueue.main.asyncAfter(deadline: .now() + Double(index * 2 + 2)) {
        if index == 3 { resize() }
        window.orderFrontRegardless()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { click(sample.0) }
      }
      DispatchQueue.main.asyncAfter(deadline: .now() + Double(index * 2 + 3)) {
        let expectedReceiver = index == 0 ? 1 : index == 1 ? 1 : index == 2 ? 2 : 3
        let expectedForeground = index == 0 ? 0 : 1
        let actualReceiver = count(".receiver"), actualForeground = count(".foreground")
        valid = valid && actualReceiver == expectedReceiver && actualForeground == expectedForeground
        print("sample \(index + 1): receiver \(actualReceiver)/\(expectedReceiver), foreground \(actualForeground)/\(expectedForeground)")
        if index == 3 { print(valid ? "PASS: cross-process delivery" : "FAIL: cross-process delivery"); finish(valid ? 0 : 1) }
      }
    }
  }
}
app.run()
