import Foundation
import HitSlopCore
import HitSlopCoreBinding
import Speech

/// Where this Mac would recognize a locale. Only `onDevice` is ever used: a locale the Mac
/// recognizes only through Apple's servers is unavailable, never a silent upload.
public enum SpeechSupport: Sendable, Equatable {
  case onDevice(Locale)
  case serverOnly(Locale)
  case unsupportedLocale
  case unavailable
}

/// The person's decision about speech recognition for hitSlop. `undetermined` after asking
/// means this build could not ask (it lacks the usage description).
public enum SpeechAuthorization: Sendable, Equatable {
  case authorized, denied, restricted, undetermined
}

/// Recognizes a recorded clip. The system recognizer in the app; tests script their own.
public protocol SpeechRecognizing: Sendable {
  /// Where `locale` (the person's own when nil) would be recognized. Never prompts.
  func support(for locale: Locale?) async -> SpeechSupport
  /// The person's decision, asking them first when they have not decided.
  func authorize() async -> SpeechAuthorization
  /// The transcript of the recording at `recording`, recognized on this Mac. Honors task
  /// cancellation.
  func transcribe(_ recording: URL, locale: Locale) async throws -> String
}

/// Speech for a slop's page: Rust admitted the request (declared `speech`, an owner's page),
/// and owns the recording's bytes, which it reads by attachment ID.
enum SpeechTranscription {
  static let cancelled = SpeechOutcome.cancelled(
    message: "Transcription stopped: the page was replaced or closed, or an export began")

  /// The outcome of transcribing the attachment `attachment` with `recognizer`. Checks
  /// support before permission, so an unsupported locale never prompts the person.
  static func run(
    _ recognizer: any SpeechRecognizing, attachment: String, locale: String?, assets: ResourceReader
  ) async -> SpeechOutcome {
    let requested = locale.map(Locale.init(identifier:))
    let name = locale ?? "your language"
    let resolved: Locale
    switch await recognizer.support(for: requested) {
    case .onDevice(let locale): resolved = locale
    case .serverOnly:
      return .unavailable(
        reason: .locale,
        message:
          "This Mac cannot transcribe \(name) on the device, and hitSlop never sends recordings elsewhere. Adding the language for Dictation in System Settings may help."
      )
    case .unsupportedLocale:
      return .unavailable(reason: .locale, message: "This Mac does not recognize \(name) speech")
    case .unavailable:
      return .unavailable(reason: .system, message: "Speech recognition is unavailable on this Mac right now")
    }
    if Task.isCancelled { return cancelled }
    switch await recognizer.authorize() {
    case .authorized: break
    case .denied:
      return .unavailable(
        reason: .permission,
        message:
          "Speech recognition is off for hitSlop; turn it on in System Settings > Privacy & Security > Speech Recognition"
      )
    case .restricted:
      return .unavailable(reason: .permission, message: "Speech recognition is restricted on this Mac")
    case .undetermined:
      return .unavailable(reason: .permission, message: "hitSlop could not ask for speech recognition permission")
    }
    if Task.isCancelled { return cancelled }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      "speech-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    do {
      // Rust verifies the recording and decides which media a recognizer may decode.
      let clip = try assets.speechClip(attachment: attachment)
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
      let recording = directory.appendingPathComponent("recording.\(clip.fileExtension)")
      try clip.bytes.write(to: recording)
      let text = try await recognizer.transcribe(recording, locale: resolved)
      if Task.isCancelled { return cancelled }
      return .transcribed(text: text, locale: resolved.identifier(.bcp47))
    } catch {
      if Task.isCancelled || error is CancellationError { return cancelled }
      return .failed(message: OwnerFailure(error).message)
    }
  }
}

extension DocumentSession {
  /// Transcribes for the attached page. The work belongs to this page: replacing or
  /// closing it, reloading its interface or starting an export cancels it, and the page
  /// receives `cancelled`.
  func serveSpeech(
    attachment: String, locale: String?, reply: @escaping @MainActor @Sendable (Any?, String?) -> Void
  ) {
    guard admitsPageServices else {
      return reply(
        encodeHostReply(
          reply: .speechTranscribe(
            outcome: .unavailable(reason: .host, message: "Speech is unavailable while the document is busy"))), nil)
    }
    let id = UUID()
    let recognizer = speechRecognizer
    let assets = owner.assets
    speechTasks[id] = Task { [weak self] in
      let outcome = await SpeechTranscription.run(recognizer, attachment: attachment, locale: locale, assets: assets)
      self?.speechTasks[id] = nil
      reply(encodeHostReply(reply: .speechTranscribe(outcome: outcome)), nil)
    }
  }

  /// Ends every transcription this page started; each replies `cancelled`.
  func cancelSpeech() {
    for task in speechTasks.values { task.cancel() }
  }
}

/// The system recognizer, on the device only (`requiresOnDeviceRecognition`). It runs on
/// every supported macOS; SpeechAnalyzer (macOS 26) is not used yet.
struct SystemSpeechRecognizer: SpeechRecognizing {
  func support(for locale: Locale?) async -> SpeechSupport {
    let recognizer = if let locale { SFSpeechRecognizer(locale: locale) } else { SFSpeechRecognizer() }
    guard let recognizer else { return .unsupportedLocale }
    guard recognizer.supportsOnDeviceRecognition else { return .serverOnly(recognizer.locale) }
    guard recognizer.isAvailable else { return .unavailable }
    return .onDevice(recognizer.locale)
  }

  func authorize() async -> SpeechAuthorization {
    // macOS terminates a process that asks without a usage description.
    guard Bundle.main.object(forInfoDictionaryKey: "NSSpeechRecognitionUsageDescription") != nil else {
      return .undetermined
    }
    var status = SFSpeechRecognizer.authorizationStatus()
    if status == .notDetermined {
      status = await withCheckedContinuation { done in
        SFSpeechRecognizer.requestAuthorization { done.resume(returning: $0) }
      }
    }
    switch status {
    case .authorized: return .authorized
    case .denied: return .denied
    case .restricted: return .restricted
    default: return .undetermined
    }
  }

  func transcribe(_ recording: URL, locale: Locale) async throws -> String {
    guard let recognizer = SFSpeechRecognizer(locale: locale) else {
      throw SlopFailure("This Mac does not recognize \(locale.identifier) speech")
    }
    let request = SFSpeechURLRecognitionRequest(url: recording)
    // Never the network: the request fails rather than leave the device.
    request.requiresOnDeviceRecognition = true
    request.shouldReportPartialResults = false
    request.addsPunctuation = true
    request.taskHint = .dictation
    let recognition = Recognition(recognizer: recognizer)
    return try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { recognition.start(request, $0) }
    } onCancel: {
      recognition.cancel()
    }
  }

  /// One recognition task and the continuation it resumes exactly once, whether it
  /// finishes, fails or is cancelled first.
  private final class Recognition: @unchecked Sendable {
    private let recognizer: SFSpeechRecognizer
    private let lock = NSLock()
    private var continuation: CheckedContinuation<String, Error>?
    private var task: SFSpeechRecognitionTask?
    private var cancelled = false
    init(recognizer: SFSpeechRecognizer) { self.recognizer = recognizer }

    func start(_ request: SFSpeechURLRecognitionRequest, _ continuation: CheckedContinuation<String, Error>) {
      lock.lock()
      guard !cancelled else {
        lock.unlock()
        return continuation.resume(throwing: CancellationError())
      }
      self.continuation = continuation
      lock.unlock()
      let task = recognizer.recognitionTask(with: request) { [self] result, error in
        if let result, result.isFinal {
          finish(.success(result.bestTranscription.formattedString))
        } else if let error {
          finish(.failure(error))
        }
      }
      lock.lock()
      self.task = task
      let cancelNow = cancelled
      lock.unlock()
      if cancelNow { task.cancel() }
    }

    func cancel() {
      lock.lock()
      cancelled = true
      let task = self.task
      lock.unlock()
      task?.cancel()
      finish(.failure(CancellationError()))
    }

    private func finish(_ result: Result<String, Error>) {
      lock.lock()
      let continuation = self.continuation
      self.continuation = nil
      lock.unlock()
      continuation?.resume(with: result)
    }
  }
}
