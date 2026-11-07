import XCTest

/// Run after PhoneEditorTests, which prepares a minimal document in the app container. Tests
/// that change what the library lists make their own uniquely named pop and delete it.
final class PhoneLifecycleTests: XCTestCase {
  @MainActor func testEdgeSwipeCanCancelAndReturnToLibrary() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    let document = app.fixture
    XCTAssertTrue(document.waitForExistence(timeout: 30), "Run PhoneEditorTests to prepare the fixture")
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
    let title = "Edge swipe " + UUID().uuidString.prefix(8)
    app.createPop(named: title)
    let edge = app.coordinate(withNormalizedOffset: CGVector(dx: 0.005, dy: 0.5))
    edge.press(
      forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)),
      withVelocity: .default, thenHoldForDuration: 0)
    // Back on Recents with the new pop's tile, never on the template it came from.
    let tile = app.buttons[title]
    XCTAssertTrue(tile.waitForExistence(timeout: 30), "Swiping back from a new pop returns to the library")
    XCTAssertTrue(app.buttons["document-actions"].waitForNonExistence(timeout: 10))
    XCTAssertFalse(app.buttons["create-pop"].exists, "Leaving a new pop never lands on its template")
    // The library is usable at once, and the new pop reopens from its tile.
    tile.tap()
    app.waitForDocument()
    app.returnToLibrary()
    // Removal waits for the close to finish saving; then the new pop is deleted.
    app.deletePops(named: title)
  }

  /// Typing, rotating with the keyboard up, and the native keyboard dismissal.
  @MainActor func testKeyboardCanBeDismissedWithoutLeavingTheDocument() throws {
    let app = XCUIApplication()
    app.launch()
    let document = app.fixture
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
    // Turning the phone with the keyboard up keeps the editor, in portrait.
    XCUIDevice.shared.orientation = .landscapeLeft
    XCTAssertTrue(input.waitForExistence(timeout: 10))
    XCTAssertLessThan(app.frame.width, app.frame.height, "The iPhone app stays in portrait")
    XCUIDevice.shared.orientation = .portrait
    let typing = XCTAttachment(screenshot: app.screenshot())
    typing.name = "iPhone editor with keyboard"
    typing.lifetime = .keepAlways
    add(typing)
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

  /// An edit survives the app going to the background, and its process being killed after
  /// that flush with the owner still open: a fresh process takes the document's lease again.
  /// Closing first would hide lifecycle recovery failures.
  @MainActor func testAnEditSurvivesBackgroundingAndAKilledProcess() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    let document = app.fixture
    XCTAssertTrue(document.waitForExistence(timeout: 30), "Run PhoneEditorTests to prepare the fixture")
    document.tap()
    app.waitForDocument()
    let input = app.webViews.textViews.firstMatch
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    let edit = " phone-ui-" + UUID().uuidString.prefix(8)
    input.tap()
    input.typeText(edit)
    // Backgrounding flushes the open document; it is still open on return.
    XCUIDevice.shared.press(.home)
    app.activate()
    XCTAssertTrue(input.waitForExistence(timeout: 15))
    XCTAssertTrue((input.value as? String)?.contains(edit) == true, "The open document survives backgrounding")
    app.terminate()
    app.launch()
    XCTAssertTrue(document.waitForExistence(timeout: 30))
    document.tap()
    app.waitForDocument()
    XCTAssertTrue(input.waitForExistence(timeout: 30))
    XCTAssertTrue((input.value as? String)?.contains(edit) == true, "The edit survives the killed process")
  }

  /// Back Up File writes the pop to Files under its name; Import from Files brings that copy
  /// back as a pop that opens, and Files shows the document's own icon.
  @MainActor func testBackupToFilesImportsAndShowsInFiles() throws {
    continueAfterFailure = false
    let app = XCUIApplication()
    app.launch()
    let title = "Files round trip " + UUID().uuidString.prefix(8)
    app.createPop(named: title)
    let tiles = app.buttons.matching(NSPredicate(format: "label == %@", title))

    app.buttons["document-actions"].tap()
    let backup = app.buttons["export-file"]
    XCTAssertTrue(backup.waitForExistence(timeout: 5))
    backup.tap()
    // The system's export picker is a separate process. Its first presentation after boot
    // takes about 15 seconds on the simulator, so wait for its action by identifier (not the
    // localized "Save") until it can be pressed, and say what was on screen if it never came.
    app.waitForSystemPicker(action: "Files export destination", attach: add)
    // The name is this run's own, so Files never asks to replace an earlier backup.
    app.buttons["DOCPicker.actionButton"].tap()
    XCTAssertTrue(app.buttons["DOCPicker.actionButton"].waitForNonExistence(timeout: 30))
    app.returnToLibrary()
    // The backup in Files is listed beside the library pop it came from.
    let exported = XCTNSPredicateExpectation(predicate: NSPredicate(format: "count == 2"), object: tiles)
    XCTAssertEqual(XCTWaiter.wait(for: [exported], timeout: 30), .completed, "The backup reached Files")

    let settings = app.buttons["Settings"]
    XCTAssertTrue(settings.waitForExistence(timeout: 15))
    settings.tap()
    let importAction = app.buttons["Import from Files"]
    XCTAssertTrue(importAction.waitForExistence(timeout: 15))
    importAction.tap()
    // The Files picker's own Browse tab, not the library's tab bar behind it.
    let backupCell = app.backup(named: title, browse: app.tabBars["DOC.browsingModeTabBar"].buttons["Browse"])
    XCTAssertTrue(backupCell.exists, "The import picker shows the backup")
    backupCell.tap()
    app.waitForDocument()
    app.returnToLibrary()
    let imported = XCTNSPredicateExpectation(predicate: NSPredicate(format: "count == 3"), object: tiles)
    XCTAssertEqual(XCTWaiter.wait(for: [imported], timeout: 30), .completed, "The imported copy joined the library")

    let files = XCUIApplication(bundleIdentifier: "com.apple.DocumentsApp")
    files.activate()
    let file = files.backup(named: title, browse: files.tabBars.buttons["Browse"])
    XCTAssertTrue(file.exists, "Files shows the backup")
    let filesIcon = XCTAttachment(screenshot: file.screenshot())
    filesIcon.name = "Custom document icon in Files"
    filesIcon.lifetime = .keepAlways
    add(filesIcon)
    app.activate()
    // A failed run leaves its pops for inspection; their names are its own.
    app.deletePops(named: title)
  }
}

// Shared navigation for lifecycle and signed-device catalog proofs.
extension XCUIApplication {
  /// The document PhoneEditorTests prepares.
  var fixture: XCUIElement { buttons["Editor fixture with icon"].firstMatch }

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
    XCTAssertTrue(actions.waitForNonExistence(timeout: 10), file: file, line: line)
  }
  /// Creates a pop named `title` from a bundled template; it opens.
  @MainActor func createPop(named title: String, file: StaticString = #filePath, line: UInt = #line) {
    tabBars.buttons["Templates"].tap()
    let template = buttons["Quick Checklist"]
    XCTAssertTrue(template.waitForExistence(timeout: 30), file: file, line: line)
    template.tap()
    let create = buttons["create-pop"]
    XCTAssertTrue(create.waitForExistence(timeout: 15), file: file, line: line)
    create.tap()
    let name = textFields["pop-name"]
    XCTAssertTrue(name.waitForExistence(timeout: 10), file: file, line: line)
    if !keyboards.firstMatch.waitForExistence(timeout: 5) { name.tap() }
    // The template's title is selected, ready to type over; clearing covers either way.
    let current = (name.value as? String)?.count ?? 0
    name.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current) + title)
    buttons["create-named-pop"].tap()
    waitForDocument(file: file, line: line)
    XCTAssertTrue(
      descendants(matching: .any)["document-page"].firstMatch.waitForExistence(timeout: 30), file: file, line: line)
  }
  /// Deletes every pop the library lists as `title`, once each has finished saving.
  @MainActor func deletePops(named title: String, file: StaticString = #filePath, line: UInt = #line) {
    if buttons["document-actions"].exists { returnToLibrary(file: file, line: line) }
    if tabBars.buttons["Recents"].exists { tabBars.buttons["Recents"].tap() }
    let tiles = buttons.matching(NSPredicate(format: "label == %@", title))
    for _ in 0..<5 {
      let tile = tiles.firstMatch
      guard tile.waitForExistence(timeout: 5) else { return }
      let count = tiles.count
      let saved = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "exists == true AND NOT (value CONTAINS 'Saving')"), object: tile)
      XCTAssertEqual(XCTWaiter.wait(for: [saved], timeout: 30), .completed, file: file, line: line)
      tile.press(forDuration: 1)
      let delete = buttons["Delete from This iPhone"]
      XCTAssertTrue(delete.waitForExistence(timeout: 10), file: file, line: line)
      delete.tap()
      let removed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "count < \(count)"), object: tiles)
      XCTAssertEqual(XCTWaiter.wait(for: [removed], timeout: 30), .completed, file: file, line: line)
    }
  }
  /// The cell of the backup named `title` in a Files browser (Files or its picker), in On My
  /// iPhone ▸ dotpop where Back Up File saves. Recents lists only what Files opened lately,
  /// so the folder is browsed: Browse shows its last place, or its locations to walk from.
  @MainActor func backup(named title: String, browse: XCUIElement) -> XCUIElement {
    let file = cells.matching(NSPredicate(format: "label BEGINSWITH %@", title)).firstMatch
    if browse.waitForExistence(timeout: 30) { browse.tap() }
    if file.waitForExistence(timeout: 5) { return file }
    for place in ["On My iPhone", "dotpop"] {
      let cell = cells.matching(NSPredicate(format: "label BEGINSWITH %@", place)).firstMatch
      if cell.waitForExistence(timeout: 10) { cell.tap() }
      if file.waitForExistence(timeout: 10) { return file }
    }
    return file
  }
  /// Waits for a system document picker's action button and records what is on screen,
  /// including when it never appears.
  @MainActor func waitForSystemPicker(
    action name: String, attach: (XCTAttachment) -> Void, file: StaticString = #filePath, line: UInt = #line
  ) {
    let ready = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "exists == true AND enabled == true AND hittable == true"),
      object: buttons["DOCPicker.actionButton"])
    let result = XCTWaiter.wait(for: [ready], timeout: 45)
    let screen = XCTAttachment(screenshot: screenshot())
    screen.name = name
    screen.lifetime = .keepAlways
    attach(screen)
    XCTAssertEqual(result, .completed, "The system picker never offered its action", file: file, line: line)
  }
}
