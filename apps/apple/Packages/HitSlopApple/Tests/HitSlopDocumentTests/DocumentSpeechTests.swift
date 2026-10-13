import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopTestSupport
import Testing
import WebKit

@testable import HitSlopDocument

/// A recognizer the test scripts: what it supports, what the person decided, and whether a
/// transcription waits until it is cancelled. It records what it was asked.
final class ScriptedSpeech: SpeechRecognizing, @unchecked Sendable {
  private struct State {
    var support = SpeechSupport.onDevice(Locale(identifier: "en-US"))
    var authorization = SpeechAuthorization.authorized
    var formats = ["audio/wav": "wav"]
    var waits = false
    var prompts = 0
    var recordings: [(bytes: Data, name: String)] = []
    var cancelled = 0
    var waiting: [AsyncStream<Void>.Continuation] = []
  }
  private let lock = NSLock()
  private var state = State()
  private func locked<T>(_ body: (inout State) -> T) -> T {
    lock.lock()
    defer { lock.unlock() }
    return body(&state)
  }
  func script(
    support: SpeechSupport = .onDevice(Locale(identifier: "en-US")), authorization: SpeechAuthorization = .authorized,
    waits: Bool = false, formats: [String: String] = ["audio/wav": "wav"]
  ) {
    locked {
      $0.support = support
      $0.authorization = authorization
      $0.waits = waits
      $0.formats = formats
    }
  }
  var prompts: Int { locked { $0.prompts } }
  var recordings: [(bytes: Data, name: String)] { locked { $0.recordings } }
  var cancelled: Int { locked { $0.cancelled } }

  func finish() {
    let waiting = locked {
      let waiting = $0.waiting
      $0.waiting.removeAll()
      return waiting
    }
    for continuation in waiting { continuation.finish() }
  }

  func fileExtension(for mediaType: String) -> String? { locked { $0.formats[mediaType] } }
  func support(for locale: Locale?) async -> SpeechSupport { locked { $0.support } }
  func authorize() async -> SpeechAuthorization {
    locked {
      $0.prompts += 1
      return $0.authorization
    }
  }
  func transcribe(_ recording: URL, locale: Locale) async throws -> String {
    let bytes = try Data(contentsOf: recording)
    let (stream, continuation) = AsyncStream<Void>.makeStream()
    let waits = locked {
      $0.recordings.append((bytes, recording.lastPathComponent))
      if $0.waits { $0.waiting.append(continuation) }
      return $0.waits
    }
    if waits {
      do {
        for await _ in stream {}
        try Task.checkCancellation()
      } catch {
        locked { $0.cancelled += 1 }
        throw error
      }
    }
    return "Buy oat milk."
  }
}

@Suite(.serialized) struct DocumentSpeechTests {
  /// A RIFF/WAVE header, which the core stores as `audio/wav`.
  static let wav = Data("RIFF".utf8) + Data([0x24, 0, 0, 0]) + Data("WAVEfmt ".utf8)
  static let png = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])

  @MainActor static func open(declaring capabilities: [String]) async throws -> (DocumentSession, ScriptedSpeech, URL) {
    let stage = try Fixtures.stage()
    try Fixtures.updateApp(stage) { $0["capabilities"] = capabilities }
    let root = try Fixtures.document(stage: stage)
    let session = try await DocumentSession.open(url: root)
    let speech = ScriptedSpeech()
    session.speechRecognizer = speech
    session.load()
    try await session.waitUntilReady()
    return (session, speech, root)
  }
  /// Stores `bytes` as an attachment through the page bridge and returns its ID.
  @MainActor static func store(_ bytes: Data, in session: DocumentSession) async throws -> String {
    let reply = try await session.webView.callAsyncJavaScript(
      """
      const put = JSON.parse(await webkit.messageHandlers.hitslop.postMessage(JSON.stringify({method:'attachments.put',bytes})));
      return put.id;
      """, arguments: ["bytes": bytes.base64EncodedString()], in: nil, contentWorld: .page)
    return try #require(reply as? String)
  }
  /// The outcome the page receives for its speech request, which always succeeds as a
  /// request.
  @MainActor static func transcribe(
    _ attachment: String, locale: String? = "en-US", in session: DocumentSession
  ) async throws -> [String: String] {
    let reply = try await session.webView.callAsyncJavaScript(
      """
      const request = {method:'speech.transcribe',attachment};
      if (locale) request.locale = locale;
      return await webkit.messageHandlers.hitslop.postMessage(JSON.stringify(request));
      """, arguments: ["attachment": attachment, "locale": locale ?? ""], in: nil, contentWorld: .page)
    let answer = try Fixtures.object(try #require(reply as? String))
    #expect(answer["ok"] as? Bool == true)
    return (answer["outcome"] as? [String: Any] ?? [:]).compactMapValues { $0 as? String }
  }

  // The page names a stored recording; the host reads it through Rust, checks support
  // before asking the person, and answers on-device recognition or why it is unavailable.
  @Test @MainActor func aDeclaringPageTranscribesStoredRecordingsOnTheDevice() async throws {
    let (session, speech, root) = try await Self.open(declaring: ["speech"])
    defer { try? FileManager.default.removeItem(at: root) }
    do {
      let clip = try await Self.store(Self.wav, in: session)
      let transcribed = try await Self.transcribe(clip, in: session)
      #expect(transcribed == ["status": "transcribed", "text": "Buy oat milk.", "locale": "en-US"])
      #expect(speech.recordings.map(\.bytes) == [Self.wav], "the recording the page stored, read by ID")
      #expect(speech.recordings.map(\.name) == ["recording.wav"])

      // A locale this Mac recognizes only through Apple's servers is unavailable, and the
      // person is not asked for permission that could not help.
      speech.script(support: .serverOnly(Locale(identifier: "fr-CA")))
      let prompts = speech.prompts
      let serverOnly = (try await Self.transcribe(clip, locale: "fr-CA", in: session))
      #expect(serverOnly["status"] == "unavailable" && serverOnly["reason"] == "locale")
      speech.script(support: .unsupportedLocale)
      #expect((try await Self.transcribe(clip, locale: nil, in: session))["reason"] == "locale")
      speech.script(support: .unavailable)
      #expect((try await Self.transcribe(clip, in: session))["reason"] == "system")
      #expect(speech.prompts == prompts)

      // A person who declined speech recognition is never transcribed for.
      for authorization in [SpeechAuthorization.denied, .restricted, .undetermined] {
        speech.script(authorization: authorization)
        let declined = (try await Self.transcribe(clip, in: session))
        #expect(declined["status"] == "unavailable" && declined["reason"] == "permission", "\(authorization)")
      }
      #expect(speech.recordings.count == 1)

      // An unsupported format never asks for permission or reaches recognition.
      speech.script()
      let promptsBeforeFormat = speech.prompts
      let photo = try await Self.store(Self.png, in: session)
      let refused = (try await Self.transcribe(photo, in: session))
      #expect(refused["status"] == "unavailable" && refused["reason"] == "format")
      #expect(refused["message"]?.contains("image/png") == true)
      #expect(speech.prompts == promptsBeforeFormat)
      #expect(speech.recordings.count == 1)

      // A different provider can accept Ogg without changing the core or page ABI.
      let ogg = Data("OggS".utf8) + Data(repeating: 0, count: 32)
      let imported = try await Self.store(ogg, in: session)
      #expect((try await Self.transcribe(imported, in: session))["reason"] == "format")
      #expect(speech.prompts == promptsBeforeFormat)
      speech.script(formats: ["audio/ogg": "ogg"])
      #expect((try await Self.transcribe(imported, in: session))["status"] == "transcribed")
      #expect(speech.recordings.last?.bytes == ogg)
      #expect(speech.recordings.last?.name == "recording.ogg")

      // Missing or damaged attachments fail before format support or permission is asked.
      let promptsBeforeMissing = speech.prompts
      let missing = try await Self.transcribe(String(repeating: "a", count: 64), in: session)
      #expect(missing["status"] == "failed")
      #expect(missing["message"]?.contains("not in this document") == true)
      let damaged = try await Self.store(Self.wav + Data([1]), in: session)
      try Fixtures.sql(root, "UPDATE attachments SET bytes=zeroblob(length(bytes)) WHERE id='\(damaged)'")
      let corrupt = try await Self.transcribe(damaged, in: session)
      #expect(corrupt["status"] == "failed")
      #expect(corrupt["message"]?.contains("damaged") == true)
      #expect(speech.prompts == promptsBeforeMissing)
      #expect(speech.recordings.count == 2)
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }

    // An app that did not declare speech never reaches the recognizer.
    let (undeclared, untouched, plain) = try await Self.open(declaring: [])
    defer { try? FileManager.default.removeItem(at: plain) }
    do {
      let clip = try await Self.store(Self.wav, in: undeclared)
      let outcome = (try await Self.transcribe(clip, in: undeclared))
      #expect(outcome["status"] == "unavailable" && outcome["reason"] == "notDeclared")
      #expect(untouched.prompts == 0 && untouched.recordings.isEmpty)
      try await undeclared.close()
    } catch {
      try? await undeclared.close()
      throw error
    }
  }

  @Test @MainActor func aTranscriptionSurvivesSavedSourceCapture() async throws {
    let (session, speech, root) = try await Self.open(declaring: ["speech"])
    defer { try? FileManager.default.removeItem(at: root) }
    do {
      speech.script(waits: true)
      let clip = try await Self.store(Self.wav, in: session)
      let pending = Task { try await Self.transcribe(clip, in: session) }
      #expect(await eventually { speech.recordings.count == 1 })
      try await session.withCaptureSnapshot { source in
        #expect(FileManager.default.fileExists(atPath: source.path))
      }
      speech.finish()
      #expect(try await pending.value["status"] == "transcribed")
      #expect(speech.cancelled == 0)
      speech.script()
      let duringCapture = try await session.withCapture { try await Self.transcribe(clip, in: session) }
      #expect(duringCapture["status"] == "transcribed")
      try await session.close()
    } catch {
      speech.finish()
      try? await session.close()
      throw error
    }
  }

  @Test(arguments: ["close", "renderer", "replace"])
  @MainActor func retiringPageServicesCancelsTranscription(reason: String) async throws {
    let (session, speech, root) = try await Self.open(declaring: ["speech"])
    defer {
      speech.finish()
      try? FileManager.default.removeItem(at: root)
    }
    do {
      speech.script(waits: true)
      let clip = try await Self.store(Self.wav, in: session)
      session.webView.callAsyncJavaScript(
        "webkit.messageHandlers.hitslop.postMessage(JSON.stringify({method:'speech.transcribe',attachment}))",
        arguments: ["attachment": clip], in: nil, in: .page, completionHandler: nil)
      #expect(await eventually { speech.recordings.count == 1 })
      switch reason {
      case "close": try await session.prepareClose()
      case "renderer": session.webViewWebContentProcessDidTerminate(session.webView)
      default: try await session.discardPending()
      }
      #expect(await eventually { speech.cancelled == 1 })
      speech.finish()
      #expect(await eventually { session.speechTasks.isEmpty })
      if reason == "close" { await session.cancelClose() }
      if reason == "renderer" { try await session.reopenSavedDocument() }
      if reason == "replace" { try await session.waitUntilReady() }
      try await session.close()
    } catch {
      try? await session.close()
      throw error
    }
  }
}
