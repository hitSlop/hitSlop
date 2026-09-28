import Foundation
import JavaScriptCore

func fixtures(_ source: String, _ url: URL) throws -> [String: Any] {
  let cases = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [[String: Any]]
  let engines = Engines(.sharedContext, source: source)
  _ = try engines.add("d0", json: "{\"title\":\"Isolated\",\"rows\":[]}")
  _ = try engines.add("d1", json: "{\"title\":\"Unchanged\",\"rows\":[]}")
  for f in cases {
    let actual = try decode(engines.call(0, "fixture", encode(f)))
    guard try encode(actual) == encode(f["expected"]!) else { throw Failure("Fixture mismatch: \(f["name"]!)") }
  }
  _ = try engines.call(0, "work", "{\"kind\":\"text\"}")
  let other = try decode(engines.call(1, "current")) as! [String: Any]
  guard (other["value"] as! [String: Any])["title"] as? String == "Unchanged" else { throw Failure("Cross-document state leak") }
  try engines.close()
  return ["fixtures": cases.count, "crossDocumentIsolation": true]
}

func headless(_ config: [String: Any], source: String, root: URL) throws -> [String: Any] {
  let topology = Topology(rawValue: config["topology"] as! String)!
  let count = config["documents"] as! Int, samples = config["samples"] as? Int ?? 30
  let seed = try String(contentsOfFile: config["seed"] as! String, encoding: .utf8)
  let before = memory(), started = clockMS()
  var engines: Engines? = Engines(topology, source: source)
  let afterVM = memory()
  var loads: [[String: Double]] = [], allocation: [[String: Double]] = []
  for i in 0..<count {
    loads.append(try engines!.add("d\(i)", json: seed))
    allocation.append(memory())
  }
  let openMS = clockMS() - started, loaded = memory()
  var results: [String: Any] = [:]
  for kind in ["checkbox", "move", "text", "batch"] {
    var applied: [Double] = [], serialized: [Double] = []
    for _ in 0..<samples {
      applied.append(try measure { _ = try engines!.call(0, "work", encode(["kind": kind, "count": kind == "batch" ? 32 : 1])) })
      serialized.append(try measure { _ = try engines!.call(0, "current") })
    }
    results[kind] = ["apply": stats(applied), "serialize": stats(serialized)]
  }
  let edited = memory(), snapshot = try engines!.call(0, "current")
  var saves: [Double] = [], durable: [Double] = []
  let store = try Store(root.appendingPathComponent("durability.sqlite"))
  for _ in 0..<samples {
    saves.append(try measure { try store.save(snapshot) })
    durable.append(try measure {
      _ = try engines!.call(0, "work", "{\"kind\":\"checkbox\"}")
      try store.save(engines!.call(0, "current"))
    })
  }
  guard try store.read() == engines!.call(0, "current") else { throw Failure("Durable snapshot mismatch") }
  store.close()
  let reopened = try Store(root.appendingPathComponent("durability.sqlite"))
  guard try reopened.read() == engines!.call(0, "current") else { throw Failure("Reopened snapshot mismatch") }
  reopened.close()
  let vmMS = engines!.vmMS
  try engines!.close(); engines = nil
  let released = memory()
  // Repeated allocation detects retained heaps without claiming immediate GC/RSS return.
  var cycles: [[String: Double]] = []
  for _ in 0..<5 {
    try autoreleasepool {
      let e = Engines(topology, source: source)
      for i in 0..<count { _ = try e.add("d\(i)", json: seed) }
      _ = try e.call(0, "work", "{\"kind\":\"checkbox\"}")
      try e.close()
    }
    cycles.append(memory())
  }
  return ["before": before, "afterVM": afterVM, "loaded": loaded, "edited": edited,
    "released": released, "afterCycles": cycles, "allocation": allocation,
    "vmMS": vmMS, "loads": loads, "openMS": openMS, "results": results,
    "snapshotBytes": snapshot.utf8.count, "saveOnly": stats(saves), "durableEdit": stats(durable),
    "finalMemory": memory(), "reopened": true]
}

func contention(_ config: [String: Any], source: String) throws -> [String: Any] {
  let e = Engines(Topology(rawValue: config["topology"] as! String)!, source: source)
  _ = try e.add("d0", json: String(contentsOfFile: config["seed"] as! String, encoding: .utf8))
  _ = try e.add("d1", json: String(contentsOfFile: config["largeSeed"] as! String, encoding: .utf8))
  var idle: [Double] = [], busy: [Double] = [], background: [Double] = [], arrival: [Double] = []
  for _ in 0..<20 {
    idle.append(try measure { _ = try e.call(0, "work", "{\"kind\":\"checkbox\"}") })
    let started = DispatchSemaphore(value: 0), finished = DispatchSemaphore(value: 0)
    var elapsed = 0.0, backgroundError: Error?
    e.slots[1].queue.async {
      let t = clockMS(); started.signal()
      do { _ = try e.slots[1].call("work", "d1", "{\"kind\":\"batch\",\"count\":1024}") }
      catch { backgroundError = error }
      elapsed = clockMS() - t; finished.signal()
    }
    started.wait()
    // Controlled arrival offset, not a completion wait. Report it; VM-lock acquisition is opaque.
    let t = clockMS(); Thread.sleep(forTimeInterval: 0.001); arrival.append(clockMS() - t)
    busy.append(try measure { _ = try e.call(0, "work", "{\"kind\":\"checkbox\"}") })
    finished.wait()
    if let backgroundError { throw backgroundError }
    background.append(elapsed)
  }
  try e.close()
  return ["idle": stats(idle), "contended": stats(busy), "background": stats(background), "arrivalDelay": stats(arrival), "memory": memory()]
}
