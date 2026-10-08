import Foundation
import CoreGraphics
import ImageIO
import HitSlopCore
import HitSlopCoreBinding

func png(_ url: URL, width: Int, height: Int) throws {
    try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    let bytes = Data(repeating: 255, count: width * height * 4)
    let provider = CGDataProvider(data: bytes as CFData)!
    let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
    let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, nil)
    if !CGImageDestinationFinalize(destination) { throw SlopPackageError.invalid("test PNG") }
}

let args = CommandLine.arguments
if args[1] == "--time" {
    let root = URL(fileURLWithPath: args[2])
    let start = DispatchTime.now().uptimeNanoseconds
    let first = try SlopPackage(rootURL: root)
    let cold = Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6
    let warmStart = DispatchTime.now().uptimeNanoseconds
    var consumed = first.manifest.slug.count
    for _ in 0..<100 {
        consumed += try autoreleasepool { try SlopPackage(rootURL: root).manifest.slug.count }
    }
    let warm = Double(DispatchTime.now().uptimeNanoseconds - warmStart) / 1e6 / 100
    print(String(decoding: try JSONSerialization.data(withJSONObject: ["cold_ms": cold, "warm_ms": warm, "consumed": consumed]), as: UTF8.self))
} else {
    let cases = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: args[1]))) as! [[String: Any]]
    let directory = URL(fileURLWithPath: args[2])
    var results: [[String: Any]] = []
    for (index, item) in cases.enumerated() {
        let input = item["input"] as! String
        let bytes = Data(input.utf8)
        let object = try JSONSerialization.jsonObject(with: bytes) as! [String: Any]
        let root = directory.appendingPathComponent("\(index).slop")
        try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
        try bytes.write(to: root.appendingPathComponent("manifest.json"))
        try Data("export default {};".utf8).write(to: root.appendingPathComponent("assets/app.js"))
        try Data(#"{"kind":"object","properties":{}}"#.utf8).write(to: root.appendingPathComponent("state.schema.json"))
        try Data("{}".utf8).write(to: root.appendingPathComponent("initial.json"))
        if let p = object["presentation"] as? [String: Any], let skin = p["skin"] as? String,
           SlopPackage.isSafeRelativePath(skin), skin.hasPrefix("assets/"),
           let width = p["width"] as? Int, let height = p["height"] as? Int,
           (1...4096).contains(width), (1...4096).contains(height) {
            try png(root.appendingPathComponent(skin), width: width, height: height)
        }
        var result: [String: Any] = ["name": item["name"]!, "package": root.path]
        #if CANDIDATE_MANIFEST_SPIKE
        do { _ = try validateManifest(manifestJson: input); result["nativeAcceptance"] = true }
        catch { result["nativeAcceptance"] = false; result["nativeError"] = error.localizedDescription }
        #else
        result["schemaAcceptance"] = PlatformContract.valid(object, against: manifestSchema)
        #endif
        do {
            let opened = try SlopPackage(rootURL: root)
            result["packageAcceptance"] = true
            result["decodedTitle"] = opened.manifest.title
        } catch {
            result["packageAcceptance"] = false
            result["packageError"] = error.localizedDescription
        }
        results.append(result)
    }
    print(String(decoding: try JSONSerialization.data(withJSONObject: results, options: [.sortedKeys]), as: UTF8.self))
}
