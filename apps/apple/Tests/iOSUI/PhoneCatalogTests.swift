import XCTest

/// Manual, signed-device artifact proof. Ordinary UI runs never create cloud records.
final class PhoneCatalogTests: XCTestCase {
  @MainActor func testBundledTemplateCreatesAnEditableSyncedSlop() throws {
    guard ProcessInfo.processInfo.environment["HITSLOP_REAL_CLOUD_PROOF"] == "1" else {
      throw XCTSkip("Requires the explicitly enabled physical-device CloudKit proof")
    }
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    app.tabBars.buttons["Templates"].tap()
    let template = app.buttons["Quick Checklist"]
    XCTAssertTrue(template.waitForExistence(timeout: 30))
    capture(app, "Templates catalog")
    template.tap()
    let create = app.buttons["create-slop"]
    XCTAssertTrue(create.waitForExistence(timeout: 15))
    capture(app, "Template preview")
    let connected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == true"), object: create)
    XCTAssertEqual(XCTWaiter.wait(for: [connected], timeout: 60), .completed, "Sign the mini into the test Apple ID")
    create.tap()
    app.alerts.buttons["Cancel"].tap()
    XCTAssertFalse(app.webViews.firstMatch.exists)
    create.tap()
    let name = app.alerts.textFields.firstMatch
    XCTAssertTrue(name.waitForExistence(timeout: 10))
    let documentName = "Phone artwork " + UUID().uuidString.prefix(6)
    name.tap()
    name.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: (name.value as? String)?.count ?? 0))
    name.typeText(String(documentName))
    app.alerts.buttons["Create"].tap()
    XCTAssertTrue(app.webViews.firstMatch.waitForExistence(timeout: 45))
    capture(app, "New synced document")
    let task = app.webViews.textFields["New task"]
    XCTAssertTrue(task.waitForExistence(timeout: 15))
    task.tap()
    task.typeText("iPhone live editor proof")
    app.webViews.buttons["Add task"].tap()
    let saved = app.webViews.descendants(matching: .any).matching(
      NSPredicate(
        format: "label CONTAINS %@ OR value CONTAINS %@", "iPhone live editor proof", "iPhone live editor proof")
    ).firstMatch
    XCTAssertTrue(saved.waitForExistence(timeout: 15))
    capture(app, "Live editor with task")
    app.buttons["document-actions"].tap()
    app.buttons["Sync Now"].tap()
    if ProcessInfo.processInfo.environment["HITSLOP_TWO_DEVICE_EDIT_PROOF"] == "1" {
      task.tap()
      task.typeText("Still typing while Mac updates")
      // Keep the real editor active while the Mac's normal owner path sends a change.
      let remote = app.webViews.descendants(matching: .any).matching(
        NSPredicate(format: "label CONTAINS %@ OR value CONTAINS %@", "Mac live editor proof", "Mac live editor proof")
      ).firstMatch
      XCTAssertTrue(
        remote.waitForExistence(timeout: 120), "Send the Mac edit through its owner while this editor remains open")
      XCTAssertTrue((task.value as? String)?.contains("Still typing while Mac updates") == true)
      capture(app, "Incoming Mac edit in open iPhone editor")
      app.webViews.buttons["Add task"].tap()
      app.buttons["document-actions"].tap()
      app.buttons["Sync Now"].tap()
    }
    app.returnToLibrary()
    XCTAssertTrue(app.buttons[String(documentName)].waitForExistence(timeout: 20))
    capture(app, "Named slop with refreshed artwork")
    if let macName = ProcessInfo.processInfo.environment["HITSLOP_MAC_DOCUMENT_NAME"] {
      let macDocument = app.buttons[macName]
      XCTAssertTrue(macDocument.waitForExistence(timeout: 30))
      macDocument.tap()
      XCTAssertTrue(app.webViews.staticTexts["Created on Mac proof"].waitForExistence(timeout: 30))
      capture(app, "Mac-created document on iPhone")
      app.returnToLibrary()
    }
    if ProcessInfo.processInfo.environment["HITSLOP_HOME_ICON_PROOF"] == "1" {
      XCUIDevice.shared.press(.home)
      let home = XCUIApplication(bundleIdentifier: "com.apple.springboard")
      home.activate()
      let icon = home.icons["hitSlop"].firstMatch
      for _ in 0..<6 {
        if icon.exists { break }
        home.swipeLeft()
      }
      let screen = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
      screen.name = "Home Screen with hitSlop"
      screen.lifetime = .keepAlways
      add(screen)
      XCTAssertTrue(icon.exists, home.debugDescription)
      let image = XCTAttachment(screenshot: icon.screenshot())
      image.name = "hitSlop Home Screen icon"
      image.lifetime = .keepAlways
      add(image)
      app.activate()
    }
  }
  @MainActor private func capture(_ app: XCUIApplication, _ name: String) {
    let image = XCTAttachment(screenshot: app.screenshot())
    image.name = name
    image.lifetime = .keepAlways
    add(image)
  }
}
