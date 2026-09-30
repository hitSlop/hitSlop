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
      let package = try SlopPackage(rootURL: root)
      let store = try storeCall { try NativeStore.open(root: root.path, mode: .document) }
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
      if phase.hasPrefix("theme:") {
        store.setPhases(phases: PhaseHook(stop))
        _ = try store.theme(defaultsJson: package.themeDefaults, change: .set(valuesJson: ##"{"accent":"#112233"}"##))
        return
      }
      let initial = String(decoding: try SlopFile.read(package.initialURL, within: root), as: UTF8.self)
      let core = try store.document(schemaKey: package.schemaKey, initialJson: initial)
      let frame = try JSONSerialization.jsonObject(with: Data(core.state().utf8)) as! [String: Any]
      let title = (frame["value"] as? [String: Any])?["title"] as? String ?? ""
      let edit = ["intents": [["type": "set", "path": ["title"], "value": "Crash edit " + title]]]
      _ = try core.applyBatch(batchJson: String(decoding: JSONSerialization.data(withJSONObject: edit), as: UTF8.self))
      store.setPhases(phases: PhaseHook(stop))
      guard let job = try core.saveJob(store: store, forceCheckpoint: phase.hasPrefix("checkpoint:")) else {
        throw failure("Probe edit produced nothing to save")
      }
      try store.write(job: job)
    }
  }
#endif
