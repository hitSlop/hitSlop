// slop-spike: the CLI. `apply`/`get` forward to a live owner or own the closed
// document in-process. Exit: 0 ok/applied, 2 rejected, 3 unknown outcome, 4 busy, 5 failed.
import Darwin
import Foundation
import NativeOwner
import OwnerService

func emit(_ object: [String: Any], exit code: Int32) -> Never {
  let data = try! JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
  FileHandle.standardOutput.write(data + Data([10]))
  exit(code)
}
let args = CommandLine.arguments
guard args.count >= 3 else {
  FileHandle.standardError.write("usage: slop-spike create|seed|get|apply DIR ...\n".data(using: .utf8)!)
  exit(64)
}
let root = URL(fileURLWithPath: args[2]).standardizedFileURL
switch args[1] {
case "create", "seed":
  // create DIR SCHEMA INITIAL  |  seed DIR SCHEMA SNAPSHOT
  let schema = try! String(contentsOfFile: args[3], encoding: .utf8)
  let checkpoint: Data =
    args[1] == "create"
    ? try! NativeDocument.create(
      schemaJson: schema, initialJson: String(contentsOfFile: args[4], encoding: .utf8)
    ).checkpoint()
    : try! Data(contentsOf: URL(fileURLWithPath: args[4]))
  let store = try! Store(root: root)
  _ = try! store.write(generation: store.metadata().generation, checkpoint: checkpoint, schemaKey: schema)
  store.close()
  emit(["status": "created"], exit: 0)
case "get", "apply":
  let started = ContinuousClock.now
  let request = Request(
    id: UUID().uuidString, method: args[1], batch: args[1] == "apply" ? args[3] : nil)
  var timings = Timings()
  // Fault injection at the real I/O boundary, used only by cli-races.ts.
  let pause = ProcessInfo.processInfo.environment["SLOP_SPIKE_PAUSE_BEFORE_COMMIT"] != nil
  let outcome = route(root: root, request: request, timings: &timings) { store in
    if pause {
      store.beforeCommit = {
        FileHandle.standardError.write("PAUSED\n".data(using: .utf8)!)
        sleep(30)
      }
    }
  }
  let d = ContinuousClock.now - started
  let total = Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
  var out: [String: Any] = [
    "routed": timings.routed, "attempts": timings.attempts, "inProcessMS": total,
    "lockMS": timings.lockMS, "openMS": timings.openMS, "applyMS": timings.applyMS,
    "flushMS": timings.flushMS,
  ]
  switch outcome {
  case .reply(let reply):
    out["status"] = reply.status
    out["body"] = reply.body
    emit(out, exit: ["ok": 0, "applied": 0, "rejected": 2][reply.status] ?? 5)
  case .unknown(let message):
    out["status"] = "unknown_outcome"
    out["body"] = message
    emit(out, exit: 3)
  case .busy(let message):
    out["status"] = "busy"
    out["body"] = message
    emit(out, exit: 4)
  }
default:
  exit(64)
}
