import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import QuickLookThumbnailing
import UIKit
import WebKit
import XCTest

@testable import HitSlopDocument
@testable import hitSlopPhone

final class PhoneEditorTests: XCTestCase {
  @MainActor func testLibraryNavigationRetainsTheDocumentWhenSaveFailsAndCanRetry() async throws {
    let file = try outputFolder().appendingPathComponent("close-\(UUID().uuidString).slop")
    let moved = file.appendingPathExtension("moved")
    _ = try SlopFile.importCopy(from: input("host-probe"), to: file)
    let (session, window) = try await show(file)
    defer { window.isHidden = true }
    let model = PhoneDocuments()
    model.session = session
    session.delegate = model
    _ = try await session.owner.apply(
      batch: #"{"intents":[{"type":"set","path":["title"],"value":"Keep my edits"}]}"#)
    try FileManager.default.moveItem(at: file, to: moved)
    defer {
      if FileManager.default.fileExists(atPath: moved.path) {
        try? FileManager.default.moveItem(at: moved, to: file)
      }
    }
    let closed = await withCheckedContinuation { reply in
      model.close { reply.resume(returning: $0) }
    }
    XCTAssertFalse(closed)
    XCTAssertTrue(model.session === session)
    XCTAssertNotNil(model.message)
    try FileManager.default.moveItem(at: moved, to: file)
    let retried = await withCheckedContinuation { reply in
      model.close { reply.resume(returning: $0) }
    }
    XCTAssertTrue(retried)
    XCTAssertNil(model.session)
    let reopened = try await DocumentSession.open(url: file)
    let saved = try await state(reopened.owner)
    XCTAssertEqual((saved["value"] as? [String: Any])?["title"] as? String, "Keep my edits")
    try await reopened.close()
  }

  @MainActor func testHapticsReachOnlyAnEnabledVisibleEditor() async throws {
    let file = try outputFolder().appendingPathComponent("haptics-\(UUID().uuidString).slop")
    _ = try SlopFile.importCopy(from: input("host-probe"), to: file)
    let prior = UserDefaults.standard.object(forKey: HapticPreferences.enabledKey)
    defer { UserDefaults.standard.set(prior, forKey: HapticPreferences.enabledKey) }
    let (session, window) = try await show(file)
    defer { window.isHidden = true }
    let feedback = RecordedHaptics()
    session.hapticFeedback = feedback
    UserDefaults.standard.set(true, forKey: HapticPreferences.enabledKey)
    let result = try await session.webView.callAsyncJavaScript(
      "return JSON.parse(await webkit.messageHandlers.hitslop.postMessage(JSON.stringify({method:'haptics.play',feedback:{kind:'selection'}}))).ok",
      arguments: [:], in: nil, contentWorld: .page)
    XCTAssertEqual(result as? Bool, true)
    XCTAssertEqual(feedback.events.count, 1)
    if case .selection? = feedback.events.first {} else { XCTFail("Expected selection feedback") }
    UserDefaults.standard.set(false, forKey: HapticPreferences.enabledKey)
    session.serveHost(.hapticsPlay(feedback: .notification(notification: .success))) { _, _ in }
    XCTAssertEqual(feedback.events.count, 1)
    UserDefaults.standard.set(true, forKey: HapticPreferences.enabledKey)
    window.isHidden = true
    session.serveHost(.hapticsPlay(feedback: .impact(style: .light))) { _, _ in }
    XCTAssertEqual(feedback.events.count, 1)
    try await session.close()
    session.serveHost(.hapticsPlay(feedback: .selection)) { _, _ in }
    XCTAssertEqual(feedback.events.count, 1)
  }
  @MainActor func testSystemThumbnailUsesDocumentArtworkWithoutModifyingTheFile() async throws {
    let file = try outputFolder().appendingPathComponent("thumbnail-\(UUID().uuidString).slop")
    _ = try SlopFile.importCopy(from: input("host-probe"), to: file)
    let before = try Data(contentsOf: file)
    let stored = try XCTUnwrap(SlopArtwork.first(file, [.icon, .preview]))
    XCTAssertEqual(stored.name, .icon)
    let request = QLThumbnailGenerator.Request(
      fileAt: file, size: CGSize(width: 96, height: 96), scale: 2, representationTypes: .thumbnail)
    let thumbnail = try await QLThumbnailGenerator.shared.generateBestRepresentation(for: request)
    XCTAssertEqual(thumbnail.type, .thumbnail)
    XCTAssertGreaterThan(thumbnail.cgImage.width, 0)
    XCTAssertEqual(try Data(contentsOf: file), before)
    let image = XCTAttachment(image: thumbnail.uiImage)
    image.name = "System thumbnail from slop icon artwork"
    image.lifetime = .keepAlways
    add(image)
  }

  private func residentBytes() -> UInt64 {
    var info = mach_task_basic_info()
    var count = mach_msg_type_number_t(MemoryLayout.size(ofValue: info) / MemoryLayout<integer_t>.size)
    let capacity = Int(count)
    let result = withUnsafeMutablePointer(to: &info) { pointer in
      pointer.withMemoryRebound(to: integer_t.self, capacity: capacity) {
        task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
      }
    }
    return result == KERN_SUCCESS ? UInt64(info.resident_size) : 0
  }
  @MainActor private func show(_ url: URL) async throws -> (DocumentSession, UIWindow) {
    let session = try await DocumentSession.open(url: url)
    let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
    let window = UIWindow(windowScene: scene)
    let controller = UIViewController()
    window.rootViewController = controller
    let view = session.webView
    view.translatesAutoresizingMaskIntoConstraints = false
    controller.view.addSubview(view)
    NSLayoutConstraint.activate([
      view.topAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.topAnchor),
      view.bottomAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.bottomAnchor),
      view.leadingAnchor.constraint(equalTo: controller.view.leadingAnchor),
      view.trailingAnchor.constraint(equalTo: controller.view.trailingAnchor),
    ])
    window.makeKeyAndVisible()
    session.load()
    try await session.waitUntilReady()
    return (session, window)
  }

  private func outputFolder() throws -> URL {
    let folder = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("ProbeExports", isDirectory: true)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    return folder
  }
  private func input(_ name: String) throws -> URL {
    try XCTUnwrap(Bundle(for: Self.self).url(forResource: name, withExtension: "slop", subdirectory: "Fixtures"))
  }
  @MainActor private func state(_ owner: DocumentOwner) async throws -> [String: Any] {
    let json = try await owner.state()
    return try XCTUnwrap(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
  }
  @MainActor private func setTheme(_ session: DocumentSession, token: String, color: String) async throws {
    _ = try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Int, Error>) in
      session.changeTheme(.set([token: color])) { reply.resume(with: $0) }
    }
    try await session.flush()
  }

  @MainActor func testRealPageEditsThemeAttachmentsAndPortableExport() async throws {
    let folder = try outputFolder()
    let document = folder.appendingPathComponent("working-\(UUID().uuidString).slop")
    _ = try SlopFile.importCopy(from: input("host-probe"), to: document)
    let (session, window) = try await show(document)
    defer { window.isHidden = true }
    let contract =
      try await session.webView.callAsyncJavaScript(
        "return await contractTest()", arguments: [:], in: nil, contentWorld: .page) as? Bool
    XCTAssertEqual(contract, true)
    _ = try await session.webView.callAsyncJavaScript(
      "const input = document.querySelector('textarea'); input.value = 'Edited on iPhone 😀'; input.dispatchEvent(new Event('input')); return true",
      arguments: [:], in: nil, contentWorld: .page)
    try await session.flush()
    let initial = try await state(session.owner)
    let value = try XCTUnwrap(initial["value"] as? [String: Any])
    XCTAssertEqual(value["title"] as? String, "Edited on iPhone 😀")
    var savesMS: [Double] = []
    for index in 0..<10 {
      _ = try await session.webView.callAsyncJavaScript(
        "const input = document.querySelector('textarea'); input.value = 'iPhone save ' + index; input.dispatchEvent(new Event('input')); return true",
        arguments: ["index": index], in: nil, contentWorld: .page)
      let started = ContinuousClock.now
      try await session.flush()
      let elapsed = started.duration(to: .now).components
      savesMS.append(Double(elapsed.seconds) * 1000 + Double(elapsed.attoseconds) / 1e15)
    }
    let hostResidentBytes = residentBytes()
    try await setTheme(session, token: "accent", color: "#cc4488")
    try await session.undo()
    let undoneTheme = try await session.currentTheme()
    XCTAssertEqual(undoneTheme.effective["accent"], "#335577")
    try await session.undo(redo: true)
    let redoneTheme = try await session.currentTheme()
    XCTAssertEqual(redoneTheme.effective["accent"], "#cc4488")
    _ = try await session.webView.callAsyncJavaScript(
      "return await iosAttachment()", arguments: [:], in: nil, contentWorld: .page)
    try await session.pauseForBackground()
    let saved = try await state(session.owner)
    let attachment = try XCTUnwrap((saved["value"] as? [String: Any])?["title"] as? String)
    XCTAssertEqual(attachment.count, 64)
    let bytes = try session.owner.assets.readRange(route: .attachment, key: attachment, offset: 0, length: 16)
    XCTAssertEqual(bytes, Data("phone attachment".utf8))
    let output = folder.appendingPathComponent("phone-export-\(UUID().uuidString).slop")
    _ = try await session.exportFile(to: output)
    XCTAssertNotNil(try SlopArtwork.first(output, [.icon]), "Files exports must retain the document icon")
    let imported = folder.appendingPathComponent("phone-reimport-\(UUID().uuidString).slop")
    _ = try SlopFile.importCopy(from: output, to: imported)
    XCTAssertNotNil(try SlopArtwork.first(imported, [.icon]), "Files import must preserve saved artwork too")
    try await session.close()
    let (reopened, reopenedWindow) = try await show(output)
    defer { reopenedWindow.isHidden = true }
    let reopenedState = try await state(reopened.owner)
    let reopenedTheme = try await reopened.currentTheme()
    XCTAssertEqual(reopenedState["value"] as? NSDictionary, saved["value"] as? NSDictionary)
    XCTAssertEqual(reopenedTheme.effective["accent"], "#cc4488")
    XCTAssertEqual(
      try reopened.owner.assets.readRange(route: .attachment, key: attachment, offset: 0, length: 16),
      Data("phone attachment".utf8))
    try await reopened.close()
    let result: [String: Any] = [
      "export": output.lastPathComponent, "attachment": attachment, "core": DocumentOwner.coreBuildID,
      "os": UIDevice.current.systemVersion, "model": UIDevice.current.model,
      "hostResidentBytesAfterSaves": hostResidentBytes,
      "pageDrainAndDurableFlushMS": savesMS,
    ]
    let report = XCTAttachment(
      data: try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted]),
      uniformTypeIdentifier: "public.json")
    report.name = "phone-editor-proof"
    report.lifetime = .keepAlways
    add(report)
    // Prepare the app container for the subsequent UI test using the same minimal
    // infrastructure fixture. No fixture or launch argument ships in the preview app.
    let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("Documents", isDirectory: true)
    let seed = support.appendingPathComponent("Editor fixture with icon.slop")
    if !FileManager.default.fileExists(atPath: seed.path) {
      _ = try SlopFile.importCopy(from: input("host-probe"), to: seed)
    }
  }

  @MainActor func testSavedArtworkRendersOnPhoneAndTracksEditsWithoutEditingState() async throws {
    _ = SlopBrand.heading(24)
    XCTAssertNotNil(UIFont(name: "Baloo2-Bold", size: 24), "The shared brand font must be bundled on iOS")
    let document = try outputFolder().appendingPathComponent("artwork-\(UUID().uuidString).slop")
    _ = try SlopFile.importCopy(from: input("host-probe"), to: document)
    let (session, window) = try await show(document)
    defer { window.isHidden = true }
    _ = try await session.owner.apply(
      batch: #"{"intents":[{"type":"set","path":["title"],"value":"First phone artwork"}]}"#)
    try await session.flush()
    let renderedFirst = try await DocumentArtworkRenderer.refresh(session: session)
    XCTAssertTrue(renderedFirst)
    let first = try XCTUnwrap(SlopArtwork.first(document, [.icon])?.png)
    _ = try await session.owner.apply(
      batch: #"{"intents":[{"type":"set","path":["title"],"value":"Updated phone artwork"}]}"#)
    try await setTheme(session, token: "accent", color: "#6633aa")
    let before = try await state(session.owner)
    let renderedSecond = try await DocumentArtworkRenderer.refresh(session: session)
    XCTAssertTrue(renderedSecond)
    let second = try XCTUnwrap(SlopArtwork.first(document, [.icon])?.png)
    XCTAssertNotEqual(first, second)
    let after = try await state(session.owner)
    XCTAssertEqual(before["version"] as? String, after["version"] as? String)
    for (name, bytes) in [
      ("Phone-generated icon", second),
      ("Phone-generated preview", try XCTUnwrap(SlopArtwork.first(document, [.preview])?.png)),
    ] {
      let attachment = XCTAttachment(data: bytes, uniformTypeIdentifier: "public.png")
      attachment.name = name
      attachment.lifetime = .keepAlways
      add(attachment)
    }
    try await session.close()
  }

  /// Opt-in shipped-artifact review; ordinary tests depend only on infrastructure fixtures.
  @MainActor func testBundledTemplateArtworkFollowsThemeOnPhone() async throws {
    guard ProcessInfo.processInfo.environment["HITSLOP_TEMPLATE_ARTWORK_PROOF"] == "1" else {
      throw XCTSkip("Enable the signed-device bundled-template artwork review explicitly")
    }
    let root = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("StarterTemplates"))
    let templates = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
      .filter { $0.pathExtension == "slop" }
    XCTAssertFalse(templates.isEmpty)
    for template in templates {
      let destination = try outputFolder().appendingPathComponent("template-artwork-\(UUID().uuidString).slop")
      _ = try SlopFile.create(from: template, to: destination)
      let original = try XCTUnwrap(SlopArtwork.first(destination, [.icon])?.png)
      let (session, window) = try await show(destination)
      let palette = try await session.currentTheme().effective.mapValues { hex in
        "#" + String(hex.dropFirst(3).prefix(4)) + String(hex.dropFirst().prefix(2)) + String(hex.dropFirst(7))
      }
      _ = try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Int, Error>) in
        session.changeTheme(.set(palette)) { reply.resume(with: $0) }
      }
      try await session.flush()
      let refreshed = try await DocumentArtworkRenderer.refresh(session: session)
      XCTAssertTrue(refreshed)
      let updated = try XCTUnwrap(SlopArtwork.first(destination, [.icon])?.png)
      XCTAssertNotEqual(original, updated)
      for role in [SlopArtwork.Name.icon, .preview] {
        let bytes = try XCTUnwrap(SlopArtwork.first(destination, [role])?.png)
        let attachment = XCTAttachment(data: bytes, uniformTypeIdentifier: "public.png")
        attachment.name = "\(template.deletingPathExtension().lastPathComponent) phone \(role.rawValue)"
        attachment.lifetime = .keepAlways
        add(attachment)
      }
      try await session.close()
      window.isHidden = true
    }
  }

  @MainActor func testReleasedDocumentsRenderEditExportAndReopen() async throws {
    let fixtures = try XCTUnwrap(
      Bundle(for: Self.self).resourceURL?.appendingPathComponent("Fixtures", isDirectory: true))
    let sources = try FileManager.default.contentsOfDirectory(at: fixtures, includingPropertiesForKeys: nil).filter {
      $0.lastPathComponent.hasPrefix("released-") && $0.pathExtension == "slop"
    }
    XCTAssertFalse(sources.isEmpty)
    let folder = try outputFolder()
    for source in sources {
      let document = folder.appendingPathComponent(UUID().uuidString + ".slop")
      _ = try SlopFile.importCopy(from: source, to: document)
      let (session, window) = try await show(document)
      let theme = try await session.currentTheme()
      let token = try XCTUnwrap(theme.effective.keys.sorted().first)
      try await setTheme(session, token: token, color: "#cc4488")
      let output = folder.appendingPathComponent(source.lastPathComponent)
      if FileManager.default.fileExists(atPath: output.path) { try FileManager.default.removeItem(at: output) }
      _ = try await session.exportFile(to: output)
      try await session.close()
      window.isHidden = true
      let (reopened, reopenedWindow) = try await show(output)
      let reopenedTheme = try await reopened.currentTheme()
      XCTAssertEqual(reopenedTheme.effective[token], "#cc4488")
      try await reopened.close()
      reopenedWindow.isHidden = true
    }
  }
}

@MainActor private final class RecordedHaptics: HapticPlaying {
  var events: [HapticFeedback] = []
  func play(_ feedback: HapticFeedback, in view: UIView) { events.append(feedback) }
}
