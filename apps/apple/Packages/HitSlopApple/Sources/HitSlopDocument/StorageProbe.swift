import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding

#if DEBUG
  /// Crash-matrix probe: prepares a real title edit with the Rust core, then pauses
  /// (SIGSTOP) at the requested storage phase so the harness can kill it there.
  public enum DebugStorageProbe {
    public static func run(_ args: [String]) throws {
      let root = URL(fileURLWithPath: args[0])
      let phase = args[1]
      let marker = URL(fileURLWithPath: args[2])
      let store = try storeCall { try NativeStore.open(path: root.path, mode: .document) }
      defer { try? store.close() }
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
      let core = try store.document()
      if phase.hasPrefix("theme:") {
        // A theme change is saved by a theme-only job.
        _ = try store.theme(change: .set(valuesJson: ##"{"accent":"#112233"}"##))
        store.setPhases(phases: PhaseHook(stop))
        guard let job = try core.saveJob(store: store, forceCheckpoint: false) else {
          throw failure("Probe theme change produced nothing to save")
        }
        try store.write(job: job)
        return
      }
      let frame = try JSONSerialization.jsonObject(with: Data(core.state().utf8)) as! [String: Any]
      let title = (frame["value"] as? [String: Any])?["title"] as? String ?? ""
      let edit = ["intents": [["type": "set", "path": ["title"], "value": "Crash edit " + title]]]
      _ = try core.applyBatch(batchJson: String(decoding: JSONSerialization.data(withJSONObject: edit), as: UTF8.self), origin: .agent)
      store.setPhases(phases: PhaseHook(stop))
      guard let job = try core.saveJob(store: store, forceCheckpoint: phase.hasPrefix("checkpoint:")) else {
        throw failure("Probe edit produced nothing to save")
      }
      try store.write(job: job)
    }
  }
#endif
