import Foundation
import JavaScriptCore
import Darwin

struct Failure: Error, CustomStringConvertible { let description: String; init(_ text: String) { description = text } }
func clockMS() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000 }
func encode(_ value: Any) throws -> String {
  String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed, .sortedKeys]), as: UTF8.self)
}
func decode(_ text: String) throws -> Any { try JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed]) }
func measure(_ work: () throws -> Void) rethrows -> Double { let start = clockMS(); try work(); return clockMS() - start }
func stats(_ values: [Double]) -> [String: Any] {
  let sorted = values.sorted()
  guard !sorted.isEmpty else { return [:] }
  return ["median": sorted[sorted.count / 2], "p95": sorted[min(sorted.count - 1, Int(ceil(Double(sorted.count) * 0.95)) - 1)], "max": sorted.last!, "samples": values]
}
func memory(_ pid: Int32 = getpid()) -> [String: Double] {
  var value = rusage_info_v4()
  let status = withUnsafeMutablePointer(to: &value) { p in
    p.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(pid, RUSAGE_INFO_V4, $0) }
  }
  guard status == 0 else { return [:] }
  return ["MiB": Double(value.ri_phys_footprint) / 1_048_576,
    "peakMiB": Double(value.ri_lifetime_max_phys_footprint) / 1_048_576]
}
enum Topology: String { case sharedVM = "shared-vm", separateVM = "separate-vm", sharedContext = "shared-context" }

/// The owning queue is explicit. Different slots may contend on a shared VM's lock.
/// Never pass JSValue across slots; no timers, DOM, authored code or native capabilities in JSC.
final class Slot {
  let context: JSContext
  private let function: JSValue
  let queue = DispatchQueue(label: "authority.jsc", qos: .userInitiated)
  var loadMS = 0.0
  init(vm: JSVirtualMachine, source: String) throws {
    let t = clockMS()
    guard let context = JSContext(virtualMachine: vm) else { throw Failure("Cannot allocate JSContext") }
    self.context = context
    context.evaluateScript(source)
    if let exception = context.exception { throw Failure(exception.toString()) }
    guard let fn = context.objectForKeyedSubscript("AuthorityCore")?.objectForKeyedSubscript("invoke"), !fn.isUndefined else {
      throw Failure("Missing bundled reducer")
    }
    function = fn
    loadMS = clockMS() - t
  }
  func call(_ method: String, _ id: String = "d0", _ input: String = "null") throws -> String {
    context.exception = nil
    let result = function.call(withArguments: [method, id, input])
    if let exception = context.exception { throw Failure(exception.toString()) }
    guard let result, let string = result.toString() else { throw Failure("Missing reducer response") }
    return string
  }
}
final class Engines {
  let topology: Topology
  private let source: String
  private var vm: JSVirtualMachine?
  var slots: [Slot] = []
  var vmMS = 0.0
  init(_ topology: Topology, source: String) {
    self.topology = topology; self.source = source
    if topology != .separateVM { let t = clockMS(); vm = JSVirtualMachine(); vmMS += clockMS() - t }
  }
  func add(_ id: String, json: String) throws -> [String: Double] {
    let slot: Slot, newSlot: Bool
    if topology == .sharedContext, let existing = slots.first { slot = existing; newSlot = false }
    else {
      let t = clockMS(), machine = vm ?? JSVirtualMachine()!
      vmMS += clockMS() - t
      slot = try Slot(vm: machine, source: source); newSlot = true
    }
    let load = try slot.queue.sync { try measure { _ = try slot.call("create", id, json) } }
    slots.append(slot)
    return ["contextAndBundleMS": newSlot ? slot.loadMS : 0, "documentLoadMS": load]
  }
  func call(_ index: Int, _ method: String, _ input: String = "null") throws -> String {
    try slots[index].queue.sync { try slots[index].call(method, "d\(index)", input) }
  }
  func close() throws {
    for i in slots.indices { _ = try call(i, "drop") }
    slots.removeAll(); vm = nil
  }
}
