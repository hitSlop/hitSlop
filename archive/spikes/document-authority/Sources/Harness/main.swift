import Foundation
import AppKit

let args = CommandLine.arguments
guard args.count == 3 else { fatalError("Usage: document-authority CONFIG.json OUTPUT.json") }
let configURL = URL(fileURLWithPath:args[1]), output = URL(fileURLWithPath:args[2])
func run() async throws {
  let config = try JSONSerialization.jsonObject(with:Data(contentsOf:configURL)) as! [String:Any]
  let source = try String(contentsOfFile:config["core"] as! String,encoding:.utf8)
  let root = output.deletingPathExtension()
  try FileManager.default.createDirectory(at:root,withIntermediateDirectories:true)
  var result: [String:Any]
  switch config["mode"] as! String {
  case "fixtures": result = try fixtures(source,URL(fileURLWithPath:config["fixtures"] as! String))
  case "headless": result = try headless(config,source:source,root:root)
  case "contention": result = try contention(config,source:source)
  case "web": result = try await web(config,source:source,root:root)
  default: throw Failure("Unknown mode")
  }
  result["config"] = config
  try Data(encode(result).utf8).write(to:output)
  print("RESULT \(output.path)")
}
let config = try JSONSerialization.jsonObject(with:Data(contentsOf:configURL)) as! [String:Any]
if config["mode"] as? String == "web" {
  let app = NSApplication.shared; app.setActivationPolicy(.accessory)
  Task { @MainActor in
    do { try await run(); exit(0) } catch { fputs("FAILED: \(error)\n",stderr); exit(1) }
  }
  app.run()
} else {
  // No NSApplication or WebKit instance for headless work.
  Task {
    do { try await run(); exit(0) } catch { fputs("FAILED: \(error)\n",stderr); exit(1) }
  }
  dispatchMain()
}
