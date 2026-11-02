import Combine
import PopCore
import PopDocument
import RiveRuntime
import SwiftUI

#if os(iOS)
  import UIKit
#else
  import AppKit
#endif

/// A decorative foreground greeting. Local work starts underneath it and never waits for Rive.
private struct LaunchGreeting: ViewModifier {
  let ready: Bool
  let enabled: Bool
  let onFinished: @MainActor () -> Void
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @StateObject private var presentation = LaunchPresentation()

  func body(content: Content) -> some View {
    content
      .overlay {
        if enabled && presentation.visible {
          GeometryReader { geometry in
            let size = min(260, max(150, min(geometry.size.width, geometry.size.height) * 0.62))
            ZStack {
              PopBrand.cream.ignoresSafeArea()
              VStack(spacing: -12) {
                ZStack {
                  if let rive = presentation.rive {
                    RiveUIViewRepresentable(rive: rive).frameRate(.fps(60))
                  } else {
                    Circle().fill(PopBrand.accent).frame(width: size * 0.075, height: size * 0.075)
                  }
                }.frame(width: size, height: size)
                Image("wordmark", bundle: .module).resizable().scaledToFit()
                  .frame(width: size * 0.46, height: size * 0.2)
                  .opacity(presentation.cue >= 2 ? 1 : 0)
                  .animation(.easeOut(duration: 0.16), value: presentation.cue >= 2)
              }
              .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .opacity(presentation.leaving ? 0 : 1)
            .animation(.easeOut(duration: 0.2), value: presentation.leaving)
          }
          .accessibilityHidden(true)
          .onTapGesture { presentation.finish() }
        }
      }
      .onAppear { begin() }
      .onChange(of: ready) { _, value in presentation.setReady(value) }
      .onChange(of: enabled, initial: true) { _, value in if !value { presentation.finish() } }
      .onChange(of: reduceMotion) { _, value in if value { presentation.finish() } }
      .onChange(of: presentation.visible) { _, value in if !value { onFinished() } }
      .onReceive(NotificationCenter.default.publisher(for: becameActiveNotification)) { _ in begin() }
      .onReceive(NotificationCenter.default.publisher(for: resignedActiveNotification)) { _ in
        if presentation.started { presentation.finish() }
      }
      .onDisappear { presentation.finish() }
  }

  private func begin() {
    guard enabled, applicationIsActive else { return }
    presentation.begin(ready: ready, reduceMotion: reduceMotion)
  }
}

extension View {
  /// Once per foreground process launch; pass only local readiness, never a network/sync status.
  public func popLaunchGreeting(
    ready: Bool, enabled: Bool = true, onFinished: @escaping @MainActor () -> Void = {}
  ) -> some View {
    modifier(LaunchGreeting(ready: ready, enabled: enabled, onFinished: onFinished))
  }
}

@MainActor private final class LaunchPresentation: ObservableObject {
  private static var presented = false
  @Published private(set) var rive: Rive?
  @Published private(set) var visible = true
  @Published private(set) var leaving = false
  @Published private(set) var cue = 0
  private(set) var started = false
  private var ready = false
  private var instance: ViewModelInstance?
  private var preparation: Task<Void, Never>?
  private var observation: Task<Void, Never>?
  private var deadline: Task<Void, Never>?
  #if os(iOS)
    private let landing = UIImpactFeedbackGenerator(style: .soft)
    private let corner = UIImpactFeedbackGenerator(style: .light)
  #endif

  func begin(ready: Bool, reduceMotion: Bool) {
    guard visible, !started else { return }
    guard !Self.presented else { return finish() }
    Self.presented = true
    guard !reduceMotion else { return finish() }
    started = true
    self.ready = ready
    #if os(iOS)
      if HapticPreferences.isEnabled {
        landing.prepare()
        corner.prepare()
      }
    #endif
    // A failed or stalled renderer must never leave a cover over the usable app.
    deadline = Task { [weak self] in
      do { try await Task.sleep(for: .seconds(5)) } catch { return }
      self?.finish()
    }
    preparation = Task { [weak self] in
      do {
        let worker = try await Worker()
        let file = try await RiveRuntime.File(source: .local("corner-hello", Bundle.module), worker: worker)
        let artboard = try await file.createArtboard()
        let instance = try await file.createViewModelInstance(.viewModelDefault(from: .artboardDefault(artboard)))
        let machine = try await artboard.createStateMachine(binding: instance)
        let rive = try await Rive(file: file, artboard: artboard, stateMachine: machine)
        try Task.checkCancellation()
        guard let self, self.visible else { return }
        self.instance = instance
        instance.setValue(of: BoolProperty(path: "ready"), to: self.ready)
        let cues = instance.valueStream(of: NumberProperty(path: "cue"))
        self.observation = Task { [weak self] in
          do {
            for try await value in cues {
              guard !Task.isCancelled, let self, self.visible else { return }
              self.receive(Int(value))
            }
          } catch {
            self?.finish()
          }
        }
        self.rive = rive
      } catch {
        if !Task.isCancelled { NSLog("Launch greeting unavailable: %@", error.localizedDescription) }
        self?.finish()
      }
    }
  }

  func setReady(_ value: Bool) {
    ready = value
    instance?.setValue(of: BoolProperty(path: "ready"), to: value)
  }

  private func receive(_ value: Int) {
    guard value > cue else { return }
    cue = value
    #if os(iOS)
      if applicationIsActive, HapticPreferences.isEnabled {
        if value == 1 { landing.impactOccurred(intensity: 0.65) }
        if value == 2 { corner.impactOccurred(intensity: 0.45) }
      }
    #endif
    if value == 3 { leaving = true }
    if value >= 4 { finish() }
  }

  func finish() {
    visible = false
    preparation?.cancel()
    observation?.cancel()
    deadline?.cancel()
    preparation = nil
    observation = nil
    deadline = nil
    rive = nil
    instance = nil
  }
}

@MainActor private var applicationIsActive: Bool {
  #if os(iOS)
    UIApplication.shared.applicationState == .active
  #else
    NSApp.isActive
  #endif
}
private var becameActiveNotification: Notification.Name {
  #if os(iOS)
    UIApplication.didBecomeActiveNotification
  #else
    NSApplication.didBecomeActiveNotification
  #endif
}
private var resignedActiveNotification: Notification.Name {
  #if os(iOS)
    UIApplication.willResignActiveNotification
  #else
    NSApplication.willResignActiveNotification
  #endif
}
