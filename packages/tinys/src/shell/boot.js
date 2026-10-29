// Visible sessions. The runtime opens the document, then mounts assets/ui.js.
try {
  const { boot } = await import("./index.js");
  await boot();
} catch (error) {
  // This file is served unbundled, so it cannot import ErrorTextLimit (4096).
  const message = String(error).slice(0, 4096);
  document.body.textContent = `Could not open this document: ${message}`;
  await globalThis.webkit?.messageHandlers?.tinys
    ?.postMessage(JSON.stringify({ method: "failed", error: message }))
    .catch(() => {});
}
