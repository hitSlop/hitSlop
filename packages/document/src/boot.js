// Visible sessions. The runtime opens the document, then mounts assets/app.js.
try {
  const { boot } = await import("./index.js");
  await boot();
} catch (error) {
  const message = String(error).slice(0, 4096);
  document.body.textContent = `Could not open this document: ${message}`;
  await globalThis.webkit?.messageHandlers?.storage.postMessage({ method: "failed", error: message }).catch(() => {});
}
