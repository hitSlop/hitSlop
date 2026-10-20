import XCTest

/// Run after PhoneEditorTests, which prepares a minimal document in the app container.
final class PhoneLifecycleTests: XCTestCase {
  @MainActor func testKeyboardRotationBackgroundRestartAndFilesRoundTrip() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    let document = app.buttons["Editor fixture with icon"]
    XCTAssertTrue(document.waitForExistence(timeout: 30), "Run the unit-test proof before the UI proof")
    document.tap()
    let input = app.webViews.textViews.firstMatch
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    let edit = " phone-ui-" + UUID().uuidString.prefix(8)
    input.tap()
    input.typeText(edit)
    XCTAssertTrue(app.keyboards.firstMatch.exists)
    XCUIDevice.shared.orientation = .landscapeLeft
    XCTAssertTrue(input.waitForExistence(timeout: 10))
    XCUIDevice.shared.orientation = .portrait
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "iPhone editor with keyboard"
    screenshot.lifetime = .keepAlways
    add(screenshot)
    XCUIDevice.shared.press(.home)
    app.activate()
    XCTAssertTrue(input.waitForExistence(timeout: 15))
    XCTAssertTrue((input.value as? String)?.contains(edit) == true)
    // Kill with the owner still open, after the background flush, then take its lease
    // again in a fresh process. Closing first would hide lifecycle recovery failures.
    app.terminate()
    app.launch()
    XCTAssertTrue(document.waitForExistence(timeout: 15))
    document.tap()
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    XCTAssertTrue((input.value as? String)?.contains(edit) == true)
    app.buttons["document-actions"].tap()
    app.buttons["export-file"].tap()
    let save = app.buttons["Save"]
    XCTAssertTrue(save.waitForExistence(timeout: 15))
    let picker = XCTAttachment(screenshot: app.screenshot())
    picker.name = "Files export destination"
    picker.lifetime = .keepAlways
    add(picker)
    save.tap()
    if app.buttons["Replace"].waitForExistence(timeout: 5) { app.buttons["Replace"].tap() }
    XCTAssertTrue(save.waitForNonExistence(timeout: 15))
    let close = app.navigationBars.buttons["Recents"]
    let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "hittable == true"), object: close)
    XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 15), .completed)
    close.tap()
    app.buttons["Settings"].tap()
    let importAction = app.buttons["Import from Files"]
    XCTAssertTrue(importAction.waitForExistence(timeout: 15))
    importAction.tap()
    if app.buttons["Recents"].waitForExistence(timeout: 3) { app.buttons["Recents"].tap() }
    let exported = app.cells.matching(NSPredicate(format: "label BEGINSWITH %@", "Editor fixture with icon")).firstMatch
    XCTAssertTrue(exported.waitForExistence(timeout: 15))
    exported.tap()
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    XCTAssertTrue((input.value as? String)?.contains(edit) == true)
    app.navigationBars.buttons["Recents"].tap()
    let files = XCUIApplication(bundleIdentifier: "com.apple.DocumentsApp")
    files.activate()
    if files.buttons["Recents"].waitForExistence(timeout: 5) { files.buttons["Recents"].tap() }
    let file = files.cells.matching(NSPredicate(format: "label BEGINSWITH %@", "Editor fixture with icon")).firstMatch
    XCTAssertTrue(file.waitForExistence(timeout: 15))
    let filesIcon = XCTAttachment(screenshot: file.screenshot())
    filesIcon.name = "Custom document icon in Files"
    filesIcon.lifetime = .keepAlways
    add(filesIcon)
    app.activate()
  }
}
