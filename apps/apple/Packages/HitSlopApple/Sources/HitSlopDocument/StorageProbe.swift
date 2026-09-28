import Darwin
import Foundation
import HitSlopCoreBinding

#if DEBUG
  /// Crash-matrix probe: prepares a real title edit with the Rust core, then pauses
  /// (SIGSTOP) at the requested storage phase so the harness can kill it there.
  public enum DebugStorageProbe {
    public static func run(_ args: [String]) throws {
      let root = URL(fileURLWithPath: args[0])
      let phase = args[1]
      let marker = URL(fileURLWithPath: args[2])
      let storage = try Storage(root: root)
      defer { storage.close() }
      let stop: (String) -> Void = { at in
        if at == phase {
          try! Data(at.utf8).write(to: marker)
          raise(SIGSTOP)
        }
      }
      if phase == "hold" {
        stop("hold")
        return
      }
      let disk = try storage.call(["method": "load"])
      guard let generation = disk["generation"] as? String, let schemaKey = disk["schemaKey"] as? String,
        let checkpoint = (disk["checkpoint"] as? String).flatMap({ Data(base64Encoded: $0) })
      else { throw failure("Probe needs a saved document") }
      let core = try NativeDocument.open(schemaJson: schemaKey, checkpoint: checkpoint)
      for update in disk["updates"] as? [String] ?? [] {
        _ = try core.importUpdates(bytes: Data(base64Encoded: update)!)
      }
      let before = try core.version()
      _ = try core.commandCurrent(batchJson: #"{"intents":[{"type":"splice","path":["title"],"index":0,"delete":0,"insert":"Crash edit "}]}"#)
      storage.testingPhase = stop
      if phase.hasPrefix("append:") {
        let bytes = try core.exportSince(version: before).base64EncodedString()
        _ = try storage.call(["method": "append", "attempt": UUID().uuidString, "generation": generation, "updates": [bytes]])
      } else {
        _ = try storage.call([
          "method": "checkpoint", "attempt": UUID().uuidString, "generation": generation,
          "bytes": try core.checkpoint().base64EncodedString(), "schemaKey": schemaKey,
        ])
      }
    }
  }
#endif
