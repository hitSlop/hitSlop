import AppKit
import HitSlopCatalog
import HitSlopCore
import HitSlopFirebase
import HitSlopHost
import HitSlopRuntime
import Sparkle
import SwiftUI
import UniformTypeIdentifiers

@main enum hitSlopApp {
    @MainActor static func main() {
        let application = NSApplication.shared
        let delegate = HitSlopAppDelegate()
        application.delegate = delegate
        withExtendedLifetime(delegate) { application.run() }
    }
}

private struct UpdateSettingsView: View {
    let updater: SPUUpdater
    var body: some View {
        Form {
            Toggle("Check for updates automatically", isOn: Binding(
                get: { updater.automaticallyChecksForUpdates },
                set: { updater.automaticallyChecksForUpdates = $0 }
            ))
        }
        .formStyle(.grouped)
        .frame(width: 420)
        .padding()
    }
}

@MainActor final class HitSlopAppDelegate: NSObject, NSApplicationDelegate, NSMenuItemValidation, NSMenuDelegate {
    private lazy var coordinator = SlopApplicationCoordinator(templatesURL: ProcessInfo.processInfo.environment["HITSLOP_TEMPLATES_ROOT"].map { URL(fileURLWithPath: $0) } ?? DocumentFactory.defaultTemplatesRoot)
    private var recentMenu: NSMenu?
    private var settingsWindow: NSWindow?
    private let updaterController = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
    var updater: SPUUpdater { updaterController.updater }

    func applicationWillFinishLaunching(_ notification: Notification) {
        HitSlopFirebase.configure()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        installMenus()
        HitSlopFirebase.telemetry.send(.launched)
        let urls = CommandLine.arguments.dropFirst().filter { $0.hasSuffix(".slop") }.map(URL.init(fileURLWithPath:))
        if urls.isEmpty {
            showCatalog()
            // A launch that opens documents warms WebKit itself.
        } else { urls.forEach(openDocument) }
    }
    func application(_ application: NSApplication, open urls: [URL]) { urls.filter { $0.isFileURL && $0.pathExtension.lowercased() == "slop" }.forEach(openDocument) }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        coordinator.requestQuit()
        return .terminateLater
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if flag { return true }
        if coordinator.hasOpenDocuments {
            coordinator.revealDocuments()
            NSApp.activate(ignoringOtherApps: true)
            return true
        }
        showCatalog()
        return true
    }
    func applicationDockMenu(_ sender: NSApplication) -> NSMenu? {
        let menu = NSMenu()
        let open = coordinator.documentControllers.sorted { $0.documentTitle.localizedStandardCompare($1.documentTitle) == .orderedAscending }
        for controller in open {
            let item = NSMenuItem(title: controller.documentTitle, action: #selector(focusDocumentFromDock(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = controller.packageURL
            item.image = controller.dockMenuImage
            menu.addItem(item)
        }
        if !open.isEmpty { menu.addItem(.separator()) }
        let catalogItem = NSMenuItem(title: "Catalog", action: #selector(showCatalog), keyEquivalent: "")
        catalogItem.target = self
        menu.addItem(catalogItem)
        return menu
    }
    @objc private func focusDocumentFromDock(_ sender: NSMenuItem) {
        guard let url = sender.representedObject as? URL else { return }
        coordinator.openDocument(url)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc func showCatalog() { coordinator.showCatalog() }
    private func openDocument(_ url: URL) { coordinator.openDocument(url) }

    @objc private func openPanel() {
        let panel = NSOpenPanel(); panel.allowedContentTypes = [.slop]; panel.allowsMultipleSelection = true
        panel.directoryURL = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask)[0]
        guard panel.runModal() == .OK else { return }; panel.urls.forEach(openDocument)
    }
    @objc private func duplicateActive() { coordinator.sendToActiveDocument(.duplicate) }
    @objc private func exportPNG() { coordinator.sendToActiveDocument(.exportPNG) }
    @objc private func exportPDF() { coordinator.sendToActiveDocument(.exportPDF) }
    @objc private func togglePin() { coordinator.sendToActiveDocument(.pin(!coordinator.isActiveDocumentPinned)) }
    @objc private func showSettings() {
        if settingsWindow == nil {
            let content = TabView {
                UpdateSettingsView(updater: updater).tabItem { Label("Updates", systemImage: "arrow.triangle.2.circlepath") }
            }.frame(width: 480, height: 340)
            let window = NSWindow(contentViewController: NSHostingController(rootView: content))
            window.title = "Settings"
            window.styleMask = [.titled, .closable]
            window.isReleasedWhenClosed = false
            window.center()
            settingsWindow = window
        }
        settingsWindow?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
    @objc private func openWebsite() { NSWorkspace.shared.open(URL(string: "https://hitslop.com")!) }
    @objc private func openRecent(_ sender: NSMenuItem) { if let url = sender.representedObject as? URL { openDocument(url) } }
    @objc private func clearRecent() { coordinator.clearRecentDocuments() }

    private func installMenus() {
        let main = NSMenu(), appItem = NSMenuItem(), fileItem = NSMenuItem(), editItem = NSMenuItem(), windowItem = NSMenuItem(), helpItem = NSMenuItem()
        main.addItem(appItem); main.addItem(fileItem); main.addItem(editItem); main.addItem(windowItem); main.addItem(helpItem)

        let app = NSMenu(title: "hitSlop"); appItem.submenu = app
        app.addItem(withTitle: "About hitSlop", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        let updates = app.addItem(withTitle: "Check for Updates…", action: #selector(SPUStandardUpdaterController.checkForUpdates(_:)), keyEquivalent: "")
        updates.target = updaterController
        app.addItem(.separator())
        item(app, "Settings…", #selector(showSettings), ",")
        app.addItem(.separator())
        let services = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        let servicesMenu = NSMenu(title: "Services")
        services.submenu = servicesMenu
        NSApp.servicesMenu = servicesMenu
        app.addItem(services)
        app.addItem(.separator())
        app.addItem(withTitle: "Hide hitSlop", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = app.addItem(withTitle: "Hide Others", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        app.addItem(withTitle: "Show All", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        app.addItem(.separator())
        app.addItem(withTitle: "Quit hitSlop", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        let file = NSMenu(title: "File"); fileItem.submenu = file
        item(file, "New from Template…", #selector(showCatalog), "n")
        item(file, "Open…", #selector(openPanel), "o")
        let recentItem = NSMenuItem(title: "Open Recent", action: nil, keyEquivalent: "")
        let recent = NSMenu(title: "Open Recent"); recent.delegate = self; recentItem.submenu = recent; recentMenu = recent
        file.addItem(recentItem)
        file.addItem(.separator())
        item(file, "Duplicate…", #selector(duplicateActive), "d")
        let export = NSMenuItem(title: "Export", action: nil, keyEquivalent: ""), exportMenu = NSMenu(title: "Export"); export.submenu = exportMenu; file.addItem(export)
        item(exportMenu, "Export PNG…", #selector(exportPNG), ""); item(exportMenu, "Export PDF…", #selector(exportPDF), "")
        file.addItem(.separator())
        file.addItem(withTitle: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")

        let edit = NSMenu(title: "Edit"); editItem.submenu = edit
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")

        let windows = NSMenu(title: "Window"); windowItem.submenu = windows
        windows.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windows.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        item(windows, "Always on Top", #selector(togglePin), "")
        windows.addItem(.separator())
        windows.addItem(withTitle: "Bring All to Front", action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")

        let help = NSMenu(title: "Help"); helpItem.submenu = help
        item(help, "hitSlop Website", #selector(openWebsite), "")

        NSApp.windowsMenu = windows
        NSApp.helpMenu = help
        NSApp.mainMenu = main
    }
    func menuNeedsUpdate(_ menu: NSMenu) {
        guard menu === recentMenu else { return }
        menu.removeAllItems()
        let urls = NSDocumentController.shared.recentDocumentURLs.filter {
            $0.pathExtension.lowercased() == "slop" && !DocumentFactory.isManagedTemplatePackage($0)
        }
        if urls.isEmpty {
            let empty = menu.addItem(withTitle: "No Recent Documents", action: nil, keyEquivalent: "")
            empty.isEnabled = false
        } else {
            for url in urls {
                let recent = menu.addItem(withTitle: url.lastPathComponent, action: #selector(openRecent(_:)), keyEquivalent: "")
                recent.representedObject = url
                recent.target = self
            }
            menu.addItem(.separator())
            let clear = menu.addItem(withTitle: "Clear Menu", action: #selector(clearRecent), keyEquivalent: "")
            clear.target = self
        }
    }
    @discardableResult private func item(_ menu: NSMenu, _ title: String, _ action: Selector, _ key: String) -> NSMenuItem { let value = menu.addItem(withTitle: title, action: action, keyEquivalent: key); value.target = self; return value }
    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if [#selector(duplicateActive), #selector(exportPNG), #selector(exportPDF), #selector(togglePin)].contains(menuItem.action) { if menuItem.action == #selector(togglePin) { menuItem.state = coordinator.isActiveDocumentPinned ? .on : .off }; return coordinator.canPerformDocumentCommands }
        return true
    }
}
