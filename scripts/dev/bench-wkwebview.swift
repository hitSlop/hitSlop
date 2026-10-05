// Loads a preview URL in a plain system WKWebView and runs the same click split
// as scripts/dev/bench-webkit.ts. Usage: swift scripts/dev/bench-wkwebview.swift [url] [rows]
import AppKit
import WebKit

let args = CommandLine.arguments
let url = URL(string: args.count > 1 ? args[1] : "http://127.0.0.1:5199/app.html")!
let rows = args.count > 2 ? Int(args[2])! : 5000

final class Runner: NSObject, WKNavigationDelegate {
  let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 480, height: 620),
    styleMask: [.titled], backing: .buffered, defer: false)
  let view = WKWebView(frame: NSRect(x: 0, y: 0, width: 480, height: 620))
  var started = Date()
  func start() {
    window.contentView = view
    window.makeKeyAndOrderFront(nil)
    view.navigationDelegate = self
    started = Date()
    view.load(URLRequest(url: url))
  }
  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    Task { @MainActor in
      let script = """
        while (document.querySelectorAll('.checklist-row').length < \(rows)) await new Promise(r => setTimeout(r, 50));
        const ready = performance.now();
        const split = { js: [], style: [], layout: [], rest: [] };
        for (let i = 0; i < 20; i++) {
          await new Promise(r => setTimeout(r, 100));
          const row = document.querySelectorAll('.checklist-row')[2];
          const changed = new Promise(res => { const o = new MutationObserver(() => { o.disconnect(); res(); }); o.observe(row, { attributes: true, subtree: true, childList: true, characterData: true }); });
          const start = performance.now();
          row.querySelector('input[type=checkbox]').click();
          await changed; await Promise.resolve(); await Promise.resolve();
          const js = performance.now();
          void getComputedStyle(document.body).color;
          const styled = performance.now();
          void document.documentElement.offsetHeight;
          const laid = performance.now();
          split.style.push(styled - js);
          await new Promise(r => setTimeout(r, 0));
          split.js.push(js - start); split.layout.push(laid - styled); split.rest.push(performance.now() - laid);
        }
        const med = a => a.sort((x, y) => x - y)[a.length >> 1];
        return JSON.stringify({ readyMS: ready, js: med(split.js), style: med(split.style), layout: med(split.layout), rest: med(split.rest) });
        """
      do {
        let result = try await view.callAsyncJavaScript(script, arguments: [:], in: nil, contentWorld: .page)
        print("system WKWebView \(rows) rows:", result ?? "nil")
      } catch { print("error:", error) }
      NSApp.terminate(nil)
    }
  }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let runner = Runner()
runner.start()
app.run()
