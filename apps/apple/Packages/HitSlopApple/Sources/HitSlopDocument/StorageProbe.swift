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
      let disk = try storage.load()
      guard let schemaKey = disk.schemaKey, let checkpoint = disk.checkpoint
      else { throw failure("Probe needs a saved document") }
      let core = try NativeDocument.open(schemaJson: schemaKey, checkpoint: checkpoint, updates: disk.updates)
      let before = try core.version()
      let frame = try JSONSerialization.jsonObject(with: Data(core.snapshot().utf8)) as! [String: Any]
      let title = (frame["value"] as? [String: Any])?["title"] as? String ?? ""
      let edit = ["intents": [["type": "set", "path": ["title"], "value": "Crash edit " + title]]]
      _ = try core.applyBatch(batchJson: String(decoding: JSONSerialization.data(withJSONObject: edit), as: UTF8.self))
      storage.testingPhase = stop
      let write: Storage.Write = phase.hasPrefix("append:")
        ? .append(try core.exportSince(version: before))
        : .checkpoint(try core.checkpoint(), schemaKey: schemaKey)
      _ = try storage.write(write, generation: disk.generation, attempt: UUID().uuidString)
    }
  }
#endif
