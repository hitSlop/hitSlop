import Foundation
import Testing
import TinysCore
import TinysDocument
import TinysTestSupport

@testable import TinysHost

// The failure screen's words, report and buttons, without a window: the window controller builds a
// `PageFailureNotice` and shows a `FailureScreen`. That the overlay appears and goes away with the
// renderer is `TinyFullscreenTests` and `WindowEditingTests`.

private let testHost = PageFailureNotice.Host(
  app: "1.3.0 (45)", core: "core-1", system: "macOS 15.2.0", architecture: "arm64", packageFormat: 7, runtimeABI: 9)

private func makeNotice(
  _ message: String = "TypeError: boom", context: TinyFailureContext = .init()
) -> PageFailureNotice {
  PageFailureNotice(
    message: message, context: context, tinyTitle: "Pomodoro", tinySlug: "pomodoro",
    packageFormat: 3, runtimeABI: 5,
    host: testHost)
}

/// What an old tiny throws while it opens: the error, then the frames WebKit lists after it.
private let oldTinyError: String = {
  let head = "TypeError: undefined is not an object (evaluating 'i.capture.isRenderer')"
  let frames = (0..<20).map { "mr@tiny://app/assets/ui.js:1:\(21217 + $0)" }
  return ([head] + frames).joined(separator: "\n")
}()

@Test func eachFailureSaysWhatHappenedInItsOwnWords() {
  let authored = makeNotice(context: .init(.authored, reason: .authoredException))
  #expect(authored.title == "This tiny couldn't open")
  #expect(
    authored.explanation
      == "Its own code failed while starting. If it opened before, a tinys update may have broken it, and we'd like to know."
  )

  let terminated = makeNotice(context: .init(reason: .webContentTerminated))
  #expect(terminated.title == "This tiny's view stopped")
  #expect(terminated.explanation.contains("quit unexpectedly"))
  #expect(terminated.explanation.contains("Reopening it usually fixes this"))
  #expect(terminated.explanation.contains("please tell us"))

  let unresponsive = makeNotice(context: .init(reason: .unresponsive))
  #expect(unresponsive.title == "This tiny stopped responding")
  #expect(
    unresponsive.explanation
      == "The view that shows this tiny stopped answering. Reopening it usually fixes this. "
      + "Edits you already made are kept; text you were still typing may be lost. If it keeps happening, please tell us."
  )

  // Every other reason keeps the original title.
  let others: [TinyFailureContext.Reason] = [.startup, .navigation, .presentation, .unknown]
  for reason in others {
    let other = makeNotice(context: .init(reason: reason))
    #expect(other.title == "This tiny stopped responding")
    #expect(other.explanation.contains("tell us if it keeps happening"))
  }
}

// The owner outlives the view and keeps every edit it accepted, so a stopped view says so. A
// tiny whose own code failed to open may have written before it threw, so it promises nothing.
@Test func onlyAStoppedViewReassuresAboutEdits() {
  #expect(
    makeNotice(context: .init(reason: .webContentTerminated)).explanation.contains("Edits you already made are kept"))
  #expect(makeNotice(context: .init(reason: .unresponsive)).explanation.contains("Edits you already made are kept"))
  let silent: [TinyFailureContext.Reason] = [.authoredException, .startup, .presentation]
  for reason in silent {
    let explanation = makeNotice(context: .init(reason: reason)).explanation.lowercased()
    #expect(!explanation.contains("kept") && !explanation.contains("saved") && !explanation.contains("safe"))
  }
}

// The notice takes no document values or paths, so the report is exactly these fixed facts and
// the page's own error text.
@Test func theReportNamesTheBuildTheTinyAndTheFailureAndNothingElse() {
  let notice = makeNotice(
    "TypeError: boom\nrender@tiny://app/assets/ui.js:1:2", context: .init(.authored, reason: .authoredException))
  #expect(
    notice.report
      == """
      tinys failure report
      tinys: 1.3.0 (45)
      Core build: core-1
      System: macOS 15.2.0, arm64
      Tiny: Pomodoro (pomodoro)
      Tiny needs: package format 3, runtime ABI 5
      This build runs: package format 7, runtime ABI 9
      Reason: authoredException
      Classification: authored

      TypeError: boom
      render@tiny://app/assets/ui.js:1:2
      """)
  #expect(makeNotice().report.contains("Reason: unknown\nClassification: platform"))
}

@Test func aLongStackIsTrimmedInTheReportAndWholeInTheDetails() {
  let notice = makeNotice(oldTinyError, context: .init(.authored, reason: .authoredException))
  #expect(notice.details == oldTinyError)
  let lines = notice.report.split(separator: "\n", omittingEmptySubsequences: false)
  let frames = lines.filter { $0.hasPrefix("mr@") }
  #expect(frames.count == PageFailureNotice.excerptLines - 1)
  #expect(notice.report.contains("TypeError: undefined is not an object (evaluating 'i.capture.isRenderer')"))
  #expect(!notice.report.contains("ui.js:1:\(21217 + 19)"))
  #expect(notice.report.hasSuffix("\n…"))

  // One endless line is bounded too.
  let endless = makeNotice(String(repeating: "x", count: 10_000), context: .init())
  #expect(endless.report.count < PageFailureNotice.excerptCharacters + 500)
}

@Test func theCurrentHostIsThisBuildOnThisMac() {
  let current = PageFailureNotice.Host.current
  #expect(current.core == DocumentOwner.coreBuildID)
  #expect(current.packageFormat == PackageFormat.level)
  #expect(current.runtimeABI == RuntimeABI.level)
  #expect(current.system.hasPrefix("macOS \(ProcessInfo.processInfo.operatingSystemVersion.majorVersion)."))
  #expect(["arm64", "x86_64"].contains(current.architecture))
}

@MainActor @Test func copyDetailsPutsTheReportOnTheClipboardAndConfirmsBriefly() async {
  var copied: [String] = []
  let screen = FailureScreen(
    notice: makeNotice(context: .init(reason: .startup)), reopen: {}, copy: { copied.append($0) }, open: { _ in },
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
    notice: makeNotice(), reopen: { reopened += 1 }, copy: { _ in }, open: { opened.append($0) })
  // The technical details start out of the way.
  #expect(!screen.showsDetails)
  screen.getHelp()
  screen.reopen()
  #expect(opened == [TinyLinks.discord])
  #expect(reopened == 1)
}
