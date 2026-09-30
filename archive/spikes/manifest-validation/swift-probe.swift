import Foundation

let schemaData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
let cases = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [[String: Any]]
let input = Data((cases[0]["input"] as! String).utf8)
let start = ProcessInfo.processInfo.systemUptime
let schema = try JSONSerialization.jsonObject(with: schemaData) as! [String: Any]
let first = PlatformContract.valid(try JSONSerialization.jsonObject(with: input), against: schema)
let cold = (ProcessInfo.processInfo.systemUptime - start) * 1e6
let warmStart = ProcessInfo.processInfo.systemUptime
var accepted = 0
for _ in 0..<1000 {
    if PlatformContract.valid(try JSONSerialization.jsonObject(with: input), against: schema) { accepted += 1 }
}
let warm = (ProcessInfo.processInfo.systemUptime - warmStart) * 1e6 / 1000
let values = try cases.map { item in
    ["name": item["name"]!, "valid": PlatformContract.valid(try JSONSerialization.jsonObject(with: Data((item["input"] as! String).utf8)), against: schema)]
}
let result: [String: Any] = ["cold_us": cold, "warm_us": warm, "first": first, "accepted": accepted, "results": values]
print(String(decoding: try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]), as: UTF8.self))
