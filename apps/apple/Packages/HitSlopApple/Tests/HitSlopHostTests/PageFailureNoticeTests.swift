import Foundation
import HitSlopCore
import HitSlopDocument
import HitSlopTestSupport
import Testing

@testable import HitSlopHost

// The failure screen's words, report and buttons, without a window: the window controller builds a
// `PageFailureNotice` and shows a `FailureScreen`. That the overlay appears and goes away with the
// renderer is `SlopFullscreenTests` and `WindowEditingTests`.

private let testHost = PageFailureNotice.Host(
  app: "1.3.0 (45)", core: "core-1", system: "macOS 15.2.0", architecture: "arm64")

private func makeNotice(
  _ message: String = "TypeError: boom", reason: SlopFailureContext.Reason?,
  classification: SlopFailureContext.Classification = .platform
) -> PageFailureNotice {
  PageFailureNotice(
    message: message, reason: reason, classification: classification, slopTitle: "Pomodoro", slopSlug: "pomodoro",
    host: testHost)
}

/// What an old slop throws while it opens: the error, then the frames WebKit lists after it.
private let oldSlopError: String = {
  let head = "TypeError: undefined is not an object (evaluating 'i.capture.isRenderer')"
  let frames = (0..<20).map { "mr@slop://app/assets/ui.js:1:\(21217 + $0)" }
  return ([head] + frames).joined(separator: "\n")
}()

@Test func eachFailureSaysWhatHappenedInItsOwnWords() {
  let authored = makeNotice(reason: .authoredException, classification: .authored)
  #expect(authored.title == "This slop couldn't open")
  #expect(
    authored.explanation
      == "Its own code failed while starting. If it opened before, a hitSlop update may have broken it, and we'd like to know."
  )

  let terminated = makeNotice(reason: .webContentTerminated)
  #expect(terminated.title == "This slop's view stopped")
  #expect(terminated.explanation.contains("quit unexpectedly"))
  #expect(terminated.explanation.contains("Reopening it usually fixes this"))
  #expect(terminated.explanation.contains("please tell us"))

  // Every other reason, and a failure without one, keeps the original title.
  let others: [SlopFailureContext.Reason?] = [.startup, .navigation, .presentation, .unknown, nil]
  for reason in others {
    let other = makeNotice(reason: reason)
    #expect(other.title == "This slop stopped responding")
    #expect(other.explanation.contains("tell us if it keeps happening"))
  }
}

// The owner outlives the view and keeps every edit it accepted, so a stopped view says so. A
// slop whose own code failed to open may have written before it threw, so it promises nothing.
@Test func onlyAStoppedViewReassuresAboutEdits() {
  #expect(makeNotice(reason: .webContentTerminated).explanation.contains("Edits you already made are kept"))
  let silent: [SlopFailureContext.Reason?] = [.authoredException, .startup, .presentation, nil]
  for reason in silent {
    let explanation = makeNotice(reason: reason).explanation.lowercased()
    #expect(!explanation.contains("kept") && !explanation.contains("saved") && !explanation.contains("safe"))
  }
}

// The notice takes no document values or paths, so the report is exactly these fixed facts and
// the page's own error text.
@Test func theReportNamesTheBuildTheSlopAndTheFailureAndNothingElse() {
  let notice = makeNotice(
    "TypeError: boom\nrender@slop://app/assets/ui.js:1:2", reason: .authoredException, classification: .authored)
  #expect(
    notice.report
      == """
      hitSlop failure report
      hitSlop: 1.3.0 (45)
      Core build: core-1
      System: macOS 15.2.0, arm64
      Slop: Pomodoro (pomodoro)
      Reason: authoredException
      Classification: authored

      TypeError: boom
      render@slop://app/assets/ui.js:1:2
      """)
  #expect(makeNotice(reason: nil).report.contains("Reason: unknown\nClassification: platform"))
}

@Test func aLongStackIsTrimmedInTheReportAndWholeInTheDetails() {
  let notice = makeNotice(oldSlopError, reason: .authoredException, classification: .authored)
  #expect(notice.details == oldSlopError)
  let lines = notice.report.split(separator: "\n", omittingEmptySubsequences: false)
  let frames = lines.filter { $0.hasPrefix("mr@") }
  #expect(frames.count == PageFailureNotice.excerptLines - 1)
  #expect(notice.report.contains("TypeError: undefined is not an object (evaluating 'i.capture.isRenderer')"))
  #expect(!notice.report.contains("ui.js:1:\(21217 + 19)"))
  #expect(notice.report.hasSuffix("\n…"))

  // One endless line is bounded too.
  let endless = makeNotice(String(repeating: "x", count: 10_000), reason: nil)
  #expect(endless.report.count < PageFailureNotice.excerptCharacters + 500)
}

@Test func theCurrentHostIsThisBuildOnThisMac() {
  let current = PageFailureNotice.Host.current
  #expect(current.core == DocumentOwner.coreBuildID)
  #expect(current.system.hasPrefix("macOS \(ProcessInfo.processInfo.operatingSystemVersion.majorVersion)."))
  #expect(["arm64", "x86_64"].contains(current.architecture))
}

@MainActor @Test func copyDetailsPutsTheReportOnTheClipboardAndConfirmsBriefly() async {
  var copied: [String] = []
  let screen = FailureScreen(
    notice: makeNotice(reason: .startup), reopen: {}, copy: { copied.append($0) }, open: { _ in },
    confirmation: .milliseconds(50))
  #expect(!screen.copied)
  screen.copyDetails()
  #expect(copied == [screen.notice.report])
  #expect(screen.copied)
  let reverted = await eventually(timeout: .seconds(2)) { !screen.copied }
  #expect(reverted)
}

@MainActor @Test func theButtonsRunTheActionsTheWindowInjects() {
  var opened: [URL] = []
  var reopened = 0
  let screen = FailureScreen(
    notice: makeNotice(reason: nil), reopen: { reopened += 1 }, copy: { _ in }, open: { opened.append($0) })
  // The technical details start out of the way.
  #expect(!screen.showsDetails)
  screen.getHelp()
  screen.reopen()
  #expect(opened == [SlopLinks.discord])
  #expect(reopened == 1)
}
