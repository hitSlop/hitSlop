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
    app.waitForDocument()
    let page = app.descendants(matching: .any)["document-page"].firstMatch
    let actions = app.buttons["document-actions"]
    XCTAssertLessThan(page.frame.minY, actions.frame.minY, "The page begins alongside native actions")
    XCTAssertFalse(app.navigationBars.buttons["Recents"].exists)
    XCTAssertFalse(document.exists, "The open document covers the library")
    let original = page.frame
    let edge = app.coordinate(withNormalizedOffset: CGVector(dx: 0.005, dy: 0.5))
    // A pull from the edge that is let go: the page follows the finger, then springs back.
    // XCUITest waits for the app to settle after a gesture, so the pulled page cannot be
    // observed here; the full swipe below, from the same edge, proves the gesture begins.
    edge.press(
      forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.5)),
      withVelocity: .slow, thenHoldForDuration: 0.3)
    // The cancelled transition settles (library removed again, page back in place, actions
    // usable) and stays settled before the next gesture can begin.
    let settled = XCTNSPredicateExpectation(
      predicate: NSPredicate { _, _ in
        !document.exists && page.exists && page.frame == original && actions.exists && actions.isHittable
      }, object: nil)
    XCTAssertEqual(XCTWaiter.wait(for: [settled], timeout: 10), .completed)
    sleep(1)
    XCTAssertEqual(page.frame, original)
    XCTAssertTrue(actions.isHittable, "Cancelling leaves the document open and usable")
    // A swipe across, released moving (with Reduce Motion a slow drag springs back).
    edge.press(
      forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)),
      withVelocity: .default, thenHoldForDuration: 0)
    XCTAssertTrue(document.waitForExistence(timeout: 30))
    XCTAssertTrue(actions.waitForNonExistence(timeout: 10))
    // The library is usable at once: reopening while the close may still be saving waits for it.
    document.tap()
    app.waitForDocument()
    app.returnToLibrary()
    XCTAssertTrue(document.waitForExistence(timeout: 30))
  }

  @MainActor func testEdgeSwipeLeavesAPopCreatedFromATemplate() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    app.tabBars.buttons["Templates"].tap()
    let template = app.buttons["Quick Checklist"]
    XCTAssertTrue(template.waitForExistence(timeout: 30))
    template.tap()
    let create = app.buttons["create-pop"]
    XCTAssertTrue(create.waitForExistence(timeout: 15))
    create.tap()
    let name = app.textFields["pop-name"]
    XCTAssertTrue(name.waitForExistence(timeout: 10))
    if !app.keyboards.firstMatch.waitForExistence(timeout: 5) { name.tap() }
    // The template's title is selected, ready to type over; clearing covers either way.
    let title = "Edge swipe " + UUID().uuidString.prefix(8)
    let current = (name.value as? String)?.count ?? 0
    name.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current) + title)
    app.buttons["create-named-pop"].tap()
    app.waitForDocument()
    XCTAssertTrue(app.descendants(matching: .any)["document-page"].firstMatch.waitForExistence(timeout: 30))
    let edge = app.coordinate(withNormalizedOffset: CGVector(dx: 0.005, dy: 0.5))
    edge.press(
      forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)),
      withVelocity: .default, thenHoldForDuration: 0)
    // Back on Recents with the new pop's tile, never on the template it came from.
    let tile = app.buttons[title]
    XCTAssertTrue(tile.waitForExistence(timeout: 30), "Swiping back from a new pop returns to the library")
    XCTAssertTrue(app.buttons["document-actions"].waitForNonExistence(timeout: 10))
    XCTAssertFalse(create.exists, "Leaving a new pop never lands on its template")
    // The library is usable at once, and the new pop reopens from its tile.
    tile.tap()
    app.waitForDocument()
    app.returnToLibrary()
    // Removal waits for the close to finish saving; then the new pop is deleted.
    let closed = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == true AND NOT (value CONTAINS 'Saving')"), object: tile)
    XCTAssertEqual(XCTWaiter.wait(for: [closed], timeout: 30), .completed)
    tile.press(forDuration: 1)
    let delete = app.buttons["Delete from This iPhone"]
    XCTAssertTrue(delete.waitForExistence(timeout: 10))
    delete.tap()
    XCTAssertTrue(tile.waitForNonExistence(timeout: 30))
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
    // The system keyboard toolbar's ✓ is the native dismissal; the page never draws its own.
    let dismiss = app.buttons["Done"].firstMatch
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
    // The Files picker's own Recents tab, not the library's behind it.
    let pickerRecents = app.tabBars["DOC.browsingModeTabBar"].buttons["Recents"]
    if pickerRecents.waitForExistence(timeout: 3) { pickerRecents.tap() }
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
