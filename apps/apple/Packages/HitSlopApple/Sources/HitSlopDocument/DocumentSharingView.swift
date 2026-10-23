import AuthenticationServices
import HitSlopAccount
import HitSlopCore
import HitSlopCoreBinding
import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

public struct LiveCollaborationSection: View {
  @ObservedObject var model: DocumentSharing
  @ObservedObject private var account = SharingAccount.shared
  @State private var wantsLink = false
  @State private var signingIn = false
  @State private var signInError: String?
  @State private var pendingStart = false
  @State private var starting = false
  @State private var confirmingStop = false
  @State private var copied = false
  @State private var copyFeedback = 0
  @State private var failedAction: Action = .refresh
  private let interactionChanged: (Bool) -> Void
  private let saveUnsharedCopy: (() -> Void)?
  private let onUpdate: (() -> Void)?

  private enum Action { case start, stop, refresh }

  public init(
    model: DocumentSharing, interactionChanged: @escaping (Bool) -> Void = { _ in },
    saveUnsharedCopy: (() -> Void)? = nil, onUpdate: (() -> Void)? = nil
  ) {
    self.model = model
    self.interactionChanged = interactionChanged
    self.saveUnsharedCopy = saveUnsharedCopy
    self.onUpdate = onUpdate
  }

  private var live: Bool { model.status.map { $0.phase != .ended } ?? false }
  private var joined: Bool { live && model.status?.creator == false }
  private var holdingOpen: Bool { model.acting || signingIn || pendingStart || confirmingStop }

  public var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Toggle(
        "Live collaboration",
        isOn: Binding(
          get: { live || wantsLink || starting },
          set: { enabled in
            if enabled {
              wantsLink = true
              if account.signedIn { start() }
            } else if live {
              interactionChanged(true)
              confirmingStop = true
            } else {
              wantsLink = false
              signInError = nil
            }
          }
        )
      )
      .toggleStyle(.switch)
      .disabled(model.busy || signingIn || joined)

      if starting && model.acting {
        HStack(spacing: 8) {
          ProgressView().controlSize(.small)
          Text("Starting live link…").font(.caption)
        }
      } else if wantsLink && (!account.signedIn || signingIn) {
        caption("Sign in with Apple to start a live link. People you invite don't need an account.")
        signInButton
      } else if model.status?.phase == .preparing {
        caption("Couldn't finish starting.")
        if model.error == nil { Button("Try Again") { start() }.disabled(model.busy) }
      } else if live {
        caption(
          joined
            ? "You joined this live document. Anyone with the link can edit."
            : "Anyone with the link can edit.")
        if let link = model.status?.link {
          HStack {
            Button(copied ? "Copied" : "Copy Link") { copy(link) }
              .accessibilityIdentifier("sharing-link")
            #if os(iOS)
              ShareLink("Share Link", item: link)
            #endif
          }
        }
        if model.status?.phase == .offline {
          caption("Offline — changes will sync when you're back online.")
        } else if model.status?.phase == .syncing {
          caption("Syncing…")
        }
      } else {
        caption(
          model.status?.phase == .ended
            ? "Live collaboration ended. Turn it on again for a new link."
            : "Edit together in real time. Anyone with the link can edit.")
      }

      if live, let kind = model.status?.pauseKind {
        switch kind {
        case .roomFull:
          caption("Making room…")
        case .outgrown, .filesFull:
          if kind == .filesFull { caption("Attachments are full.") }
          if joined {
            caption("Ask for a new link, or save an unshared copy.")
          } else {
            caption("This document can't keep syncing. Save an unshared copy to keep editing.")
          }
        case .needsUpdate:
          caption("This shared document needs a newer hitSlop.")
          if let onUpdate { Button("Update hitSlop", action: onUpdate) }
        case .missingHistory, .refused:
          caption(model.status?.detail ?? "This copy could not sync. Save an unshared copy to keep editing.")
        case .ended:
          caption("Live collaboration ended. You keep your copy.")
        }
      }
      if live, model.status?.pauseKind != .roomFull, let saveUnsharedCopy {
        Button("Save Unshared Copy…", action: saveUnsharedCopy).disabled(model.busy)
        caption("The copy is an independent document. To share it, turn on live collaboration in the copy.")
      }
      if confirmingStop {
        caption("Stop live collaboration? The link stops working. Everyone keeps their own copy.")
        HStack {
          Button("Cancel") { confirmingStop = false }
          Button("Stop", role: .destructive) {
            confirmingStop = false
            wantsLink = false
            failedAction = .stop
            model.stop()
          }
        }.disabled(model.busy)
      }
      if let error = model.error ?? signInError {
        Text(error).font(.caption).foregroundStyle(.red).textSelection(.enabled)
        Button("Try Again") {
          if signInError != nil {
            signInError = nil
            wantsLink = true
            if account.signedIn { start() }
          } else if failedAction == .stop {
            model.stop()
          } else if (failedAction == .start && !live) || model.status?.phase == .preparing {
            start()
          } else {
            model.retry()
          }
        }.disabled(model.busy || signingIn)
      }
      if account.signedIn {
        HStack(spacing: 4) {
          Text("Signed in with Apple ·")
          Button("Sign Out") { account.signOut() }.buttonStyle(.plain).foregroundStyle(.tint)
            .disabled(model.busy || signingIn)
        }.font(.caption).foregroundStyle(.secondary)
      }
    }
    .fixedSize(horizontal: false, vertical: true)
    .appHaptic(.success, trigger: copyFeedback)
    .appHaptic(.success, trigger: model.acting) { wasActing, acting in
      wasActing && !acting && model.error == nil
    }
    .appHaptic(.error, trigger: model.error) { _, error in error != nil }
    .appHaptic(.error, trigger: signInError) { _, error in error != nil }
    .task { account.configure() }
    .onChange(of: holdingOpen) { _, value in interactionChanged(value) }
    .onChange(of: model.busy) { _, busy in
      if !busy {
        if starting { wantsLink = false }
        starting = false
        if model.error == nil { failedAction = .refresh }
        startWhenReady()
      }
    }
    .onChange(of: model.status?.phase) { _, phase in
      if phase == .ended {
        wantsLink = false
        confirmingStop = false
      }
    }
  }

  @ViewBuilder private var signInButton: some View {
    #if os(macOS)
      SharingAppleSignInButton {
        interactionChanged(true)
        signingIn = true
        signInError = nil
        Task {
          let success = await account.complete()
          signingIn = false
          if success {
            pendingStart = true
            startWhenReady()
          } else if !account.message.isEmpty {
            signInError = account.message
          }
        }
      }
      .disabled(signingIn)
    #else
      SignInWithAppleButton(.continue) { request in
        interactionChanged(true)
        signingIn = true
        signInError = nil
        account.prepare(request)
      } onCompletion: { result in
        Task {
          let success = await account.complete(result)
          signingIn = false
          if success {
            pendingStart = true
            startWhenReady()
          } else {
            signInError = account.message
          }
        }
      }
      .frame(height: signInHeight)
      .disabled(signingIn)
    #endif
  }

  private var signInHeight: CGFloat {
    #if os(macOS)
      32
    #else
      44
    #endif
  }

  private func caption(_ text: String) -> some View {
    Text(text).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
  }

  private func start() {
    failedAction = .start
    wantsLink = true
    starting = model.create()
  }

  private func startWhenReady() {
    guard pendingStart, !model.busy else { return }
    pendingStart = false
    start()
  }

  private func copy(_ link: String) {
    #if os(macOS)
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(link, forType: .string)
    #else
      UIPasteboard.general.string = link
    #endif
    copyFeedback += 1
    copied = true
    Task {
      try? await Task.sleep(for: .seconds(2))
      copied = false
    }
  }
}

public struct SharingJoinView: View {
  public let join: @MainActor (String) async throws -> Void
  @State private var link = ""
  @State private var busy = false
  @State private var error: String?
  @Environment(\.dismiss) private var dismiss
  public init(join: @escaping @MainActor (String) async throws -> Void) { self.join = join }
  public var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Text("Open a Shared Link").font(.title2.bold())
      TextField("Paste a hitSlop link", text: $link).textFieldStyle(.roundedBorder)
        .accessibilityIdentifier("sharing-join-link")
      if let error { Text(error).foregroundStyle(.red).textSelection(.enabled) }
      if busy { ProgressView() }
      HStack {
        Button("Cancel", role: .cancel) { dismiss() }
        Spacer()
        Button("Open") {
          busy = true
          error = nil
          Task {
            defer { busy = false }
            do {
              try await join(link.trimmingCharacters(in: .whitespacesAndNewlines))
              dismiss()
            } catch { self.error = sharingMessage(error) }
          }
        }.disabled(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
      }
    }.padding(24).frame(idealWidth: 440).disabled(busy)
      .appHaptic(.error, trigger: error) { _, error in error != nil }
  }
}
