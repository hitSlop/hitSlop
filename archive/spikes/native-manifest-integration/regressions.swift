
// Shared with the baseline: these acceptance regressions must fail before extraction.
@Test func nativeManifestParityRegressions() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    let original = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    for count in [40, 41, 80, 81] {
        var value = original
        value["title"] = String(repeating: "😀", count: count)
        try JSONSerialization.data(withJSONObject: value).write(to: url)
        let accepted = (try? SlopPackage(rootURL: root)) != nil
        #expect(accepted == (count <= 80), "title with \(count) emoji")
    }
    var value = original
    value["author"] = ["name": "\u{0085}"]
    try JSONSerialization.data(withJSONObject: value).write(to: url)
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    value["author"] = ["name": "Author", "url": "https://[bad]"]
    try JSONSerialization.data(withJSONObject: value).write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    value = original
    value["presentation"] = ["width": 320, "height": 240, "skin": "assets/skin.png\n"]
    try writeSkin(to: root.appendingPathComponent("assets/skin.png\n"), width: 320, height: 240)
    try JSONSerialization.data(withJSONObject: value).write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}

@Test func manifestInputBoundaryRemainsStrict() throws {
    let root = try fixture()
    defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    let original = try Data(contentsOf: url)
    for invalid in [Data([0xff]), Data("{".utf8)] {
        try invalid.write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
    var boundary = original
    boundary.append(Data(repeating: 32, count: 65536 - original.count))
    try boundary.write(to: url)
    #expect(throws: Never.self) { _ = try SlopPackage(rootURL: root) }
    boundary.append(32)
    try boundary.write(to: url)
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
}
