// Distinct boundary: actual generated Swift bindings and native byte lifetimes.
// Literal fixture outcomes are checked here, not delegated back to Rust tests.
import Foundation
import HitSlopCoreBinding

struct CheckFailure: Error { let message: String }
func check(_ condition: Bool, _ message: String) throws {
  if !condition { throw CheckFailure(message: message) }
}
func json(_ value: Any) throws -> String {
  String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed]), as: UTF8.self)
}
func decode(_ text: String) throws -> [String: Any] {
  try JSONSerialization.jsonObject(with: Data(text.utf8)) as! [String: Any]
}
func equal(_ a: Any, _ b: Any) -> Bool { (a as AnyObject).isEqual(b) }
func snapshot(_ doc: NativeDocument) throws -> [String: Any] { try decode(doc.snapshot()) }

func applyPatch(_ value: Any, _ path: [Any], _ op: [String: Any]) throws -> Any {
  if let head = path.first {
    if let key = head as? String {
      var object = value as! [String: Any]
      if path.count == 1 && op["type"] as? String == "remove" { object.removeValue(forKey: key) }
      else { object[key] = try applyPatch(object[key]!, Array(path.dropFirst()), op) }
      return object
    }
    let id = (head as! [String: String])["id"]!
    var rows = value as! [[String: Any]]
    let i = rows.firstIndex { $0["$id"] as? String == id }!
    rows[i] = try applyPatch(rows[i], Array(path.dropFirst()), op) as! [String: Any]
    return rows
  }
  if op["type"] as? String == "set" { return op["value"]! }
  var rows = value as! [[String: Any]]
  if op["type"] as? String == "insertRow" { rows.insert(op["value"] as! [String: Any], at: op["index"] as! Int) }
  else {
    let i = rows.firstIndex { equal($0["$id"]!, op["id"]!) }!
    let row = rows.remove(at: i)
    if op["type"] as? String == "moveRow" { rows.insert(row, at: op["index"] as! Int) }
    else { try check(op["type"] as? String == "deleteRow", "unknown patch") }
  }
  return rows
}

do {
  let fixtureURL = URL(fileURLWithPath: CommandLine.arguments[1])
  let fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as! [String: Any]
  let schema = try json(fixture["schema"]!)
  let initial = fixture["initial"]!
  let cases = fixture["scenarios"] as! [[String: Any]]
  var names: [String] = []
  for item in cases {
    let name = item["name"] as! String
    let doc = try NativeDocument.create(schemaJson: schema, initialJson: json(item["initial"] ?? initial))
    let before = try snapshot(doc)
    let version = try doc.version()
    let seed = try doc.checkpoint()
    let intents = (item["intents"] as! [[String: Any]]).map { input in
      var op = input
      if op["base"] as? String == "$current" { op["base"] = version }
      return op
    }
    if let error = item["error"] as? String {
      var caught: Error?
      do { _ = try doc.apply(batchJson: json(["intents": intents])) } catch { caught = error }
      try check(caught.map { String(describing: $0).contains(error) } ?? false, "\(name): wrong/missing error: \(String(describing: caught))")
      try check(equal(try snapshot(doc), before), "\(name): rejected batch changed state/version/publication")
    } else {
      let reply = try decode(doc.apply(batchJson: json(["intents": intents])))
      let after = try snapshot(doc)
      try check(equal(after["value"]!, item["after"]!), "\(name): wrong value")
      let patch = reply["patch"] as! [String: Any]
      var value = before["value"]!
      for op in patch["ops"] as! [[String: Any]] { value = try applyPatch(value, op["path"] as! [Any], op) }
      try check(equal(value, item["after"]!), "\(name): patch diverged")
      try check(equal(patch["issues"]!, after["issues"]!), "\(name): issues diverged")
      let reopened = try NativeDocument.open(schemaJson: schema, checkpoint: seed)
      _ = try reopened.importUpdates(bytes: doc.exportSince(version: version))
      try check(equal(try snapshot(reopened)["value"]!, item["after"]!), "\(name): incremental replay")
      let full = try NativeDocument.open(schemaJson: schema, checkpoint: doc.checkpoint())
      try check(equal(try snapshot(full)["value"]!, item["after"]!), "\(name): checkpoint replay")
    }
    names.append(name)
  }
  // Import an independently produced WASM checkpoint + delta through native FFI.
  if CommandLine.arguments.count > 2 {
    let transfer = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [String: Any]
    let doc = try NativeDocument.open(schemaJson: schema, checkpoint: Data(base64Encoded: transfer["checkpoint"] as! String)!)
    _ = try doc.importUpdates(bytes: Data(base64Encoded: transfer["update"] as! String)!)
    try check(equal(try snapshot(doc)["value"]!, transfer["expected"]!), "WASM → native replay")
  }
  let doc = try NativeDocument.create(schemaJson: schema, initialJson: json(initial))
  let seed = try doc.checkpoint()
  let version = try doc.version()
  _ = try doc.apply(batchJson: json(["intents": cases[0]["intents"]!]))
  let view = try snapshot(doc)
  let output: [String: Any] = ["scenarios": names, "checkpoint": seed.base64EncodedString(), "update": try doc.exportSince(version: version).base64EncodedString(), "expected": view["value"]!]
  print(try json(output))
} catch {
  FileHandle.standardError.write(Data("Native conformance failed: \(error)\n".utf8))
  exit(1)
}
