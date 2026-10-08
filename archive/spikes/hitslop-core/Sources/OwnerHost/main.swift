// owner-host: an app stand-in that keeps one document open and serves the socket.
// Options (milliseconds): --delay-listen (hold the lock before listening),
// --edit-every (append "h" to the title, as a user typing), --pause-before-reply,
// --slow-close (after the final flush, before releasing the lock).
import Darwin
import Foundation
import NativeOwner
import OwnerService

setvbuf(stdout, nil, _IOLBF, 0)
let args = CommandLine.arguments
let root = URL(fileURLWithPath: args[1]).standardizedFileURL
func option(_ name: String) -> UInt32 {
  guard let i = args.firstIndex(of: name), i + 1 < args.count else { return 0 }
  return UInt32(args[i + 1]) ?? 0
}
var hostTimer: DispatchSourceTimer?
let store = try Store(root: root)
let owner = try RustOwner(store: store)
let server = OwnerServer(root: root, store: store, owner: owner)
if option("--delay-listen") > 0 {
  print("LOCKED")
  usleep(option("--delay-listen") * 1000)
}
if option("--pause-before-reply") > 0 {
  let pause = option("--pause-before-reply")
  server.beforeReply = { request in
    print("APPLIED \(request.id)")
    usleep(pause * 1000)
  }
}
try server.start()
print("READY")
if option("--edit-every") > 0 {
  let timer = DispatchSource.makeTimerSource(queue: server.queue)
  let every = Int(option("--edit-every"))
  timer.schedule(deadline: .now(), repeating: .milliseconds(every))
  timer.setEventHandler {
    let edit = #"{"intents":[{"type":"splice","path":["title"],"index":0,"delete":0,"insert":"h"}]}"#
    if (try? owner.command(edit)) != nil { try? owner.flush() }
  }
  timer.resume()
  hostTimer = timer
}
signal(SIGTERM, SIG_IGN)
let term = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
term.setEventHandler {
  hostTimer?.cancel()
  let slow = option("--slow-close")
  do {
    try server.close { if slow > 0 { print("RELEASING"); usleep(slow * 1000) } }
    print("CLOSED")
    exit(0)
  } catch {
    print("CLOSE FAILED \(error)")
    exit(5)
  }
}
term.resume()
dispatchMain()
