import XCTest

/// Run after PhoneEditorTests, which prepares a minimal document in the app container.
final class PhoneLifecycleTests: XCTestCase {
  @MainActor func testEdgeSwipeCanCancelAndReturnToLibrary() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    let document = app.buttons["Editor fixture with icon"]
    XCTAssertTrue(document.waitForExistence(timeout: 30))
    document.tap()
    let page = app.descendants(matching: .any)["document-page"].firstMatch
    XCTAssertTrue(page.waitForExistence(timeout: 30))
    let actions = app.buttons["document-actions"]
    let actionsReady = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == true AND enabled == true AND hittable == true"), object: actions)
    XCTAssertEqual(XCTWaiter.wait(for: [actionsReady], timeout: 20), .completed)
    XCTAssertLessThan(page.frame.minY, actions.frame.minY, "The page begins alongside native actions")
    XCTAssertFalse(app.navigationBars.buttons["Recents"].exists)
    let originalPage = page.frame
    let edge = app.coordinate(withNormalizedOffset: CGVector(dx: 0.005, dy: 0.5))
    edge.press(
      forDuration: 0.05,
      thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.12, dy: 0.5)),
      withVelocity: .slow, thenHoldForDuration: 0.2)
    let settled = XCTNSPredicateExpectation(
      predicate: NSPredicate { _, _ in page.exists && abs(page.frame.minX - originalPage.minX) < 1 }, object: nil)
    XCTAssertEqual(XCTWaiter.wait(for: [settled], timeout: 10), .completed)
    XCTAssertTrue(actions.isHittable)
    edge.press(
      forDuration: 0.05,
      thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)),
      withVelocity: .slow, thenHoldForDuration: 0.2)
    XCTAssertTrue(document.waitForExistence(timeout: 30))
    document.tap()
    XCTAssertTrue(page.waitForExistence(timeout: 30))
    app.returnToLibrary()
    XCTAssertTrue(document.waitForExistence(timeout: 30))
  }

  @MainActor func testKeyboardCanBeDismissedWithoutLeavingTheDocument() throws {
    let app = XCUIApplication()
    app.launch()
    let document = app.buttons["Editor fixture with icon"]
    XCTAssertTrue(document.waitForExistence(timeout: 30), "Run PhoneEditorTests to prepare the fixture")
    document.tap()
    app.waitForDocument()
    let input = app.webViews.textViews.firstMatch
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    let originalWidth = input.frame.width
    input.tap()
    let edit = " mobile-keyboard-" + UUID().uuidString.prefix(8)
    input.typeText(edit)
    XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 10))
    XCTAssertEqual(input.frame.width, originalWidth, accuracy: 1, "Focusing a small input must not zoom the page")
    let dismiss = app.buttons["dismiss-keyboard"]
    XCTAssertTrue(dismiss.waitForExistence(timeout: 5), "Every document needs a native keyboard dismissal action")
    dismiss.tap()
    XCTAssertTrue(app.keyboards.firstMatch.waitForNonExistence(timeout: 10))
    XCTAssertTrue((input.value as? String)?.contains(edit) == true)
    let page = app.descendants(matching: .any)["document-page"].firstMatch
    XCTAssertTrue(page.exists)
    page.pinch(withScale: 2, velocity: 1)
    XCTAssertEqual(input.frame.width, originalWidth, accuracy: 1, "Pinching must not scale the editor")
    XCUIDevice.shared.orientation = .landscapeLeft
    XCTAssertLessThan(app.frame.width, app.frame.height, "The iPhone app stays in portrait")
    XCUIDevice.shared.orientation = .portrait
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Portrait editor after typing and dismissing keyboard"
    screenshot.lifetime = .keepAlways
    add(screenshot)
  }

  @MainActor func testKeyboardRotationBackgroundRestartAndFilesRoundTrip() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    let document = app.buttons["Editor fixture with icon"]
    XCTAssertTrue(document.waitForExistence(timeout: 30), "Run the unit-test proof before the UI proof")
    document.tap()
    app.waitForDocument()
    let input = app.webViews.textViews.firstMatch
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    let edit = " phone-ui-" + UUID().uuidString.prefix(8)
    input.tap()
    input.typeText(edit)
    XCTAssertTrue(app.keyboards.firstMatch.exists)
    XCUIDevice.shared.orientation = .landscapeLeft
    XCTAssertTrue(input.waitForExistence(timeout: 10))
    XCTAssertLessThan(app.frame.width, app.frame.height, "The iPhone app stays in portrait")
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
    app.waitForDocument()
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
    app.returnToLibrary()
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
    app.returnToLibrary()
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

// Shared navigation for lifecycle and signed-device catalog proofs.
extension XCUIApplication {
  @MainActor func waitForDocument(file: StaticString = #filePath, line: UInt = #line) {
    let ready = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == true AND enabled == true AND hittable == true"),
      object: buttons["document-actions"])
    XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 45), .completed, file: file, line: line)
  }
  @MainActor func returnToLibrary(file: StaticString = #filePath, line: UInt = #line) {
    let actions = buttons["document-actions"]
    let ready = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == true AND enabled == true AND hittable == true"), object: actions)
    XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 20), .completed, file: file, line: line)
    actions.tap()
    let back = buttons["back-to-library"]
    XCTAssertTrue(back.waitForExistence(timeout: 5), file: file, line: line)
    back.tap()
  }
}
