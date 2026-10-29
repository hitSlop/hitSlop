import { BrowserHost } from "../schema/constants";
import { WorkerOwner } from "./worker-owner";

const owner = new WorkerOwner({
  async importCopy(installed, request) {
    if (request.source) {
      const url = `/__import/${encodeURIComponent(request.source)}`;
      const head = await fetch(url, { method: "HEAD" });
      if (!head.ok) throw new Error("Import expired. Run tiny open --browser again.");
      const size = Number(head.headers.get("Content-Length"));
      if (!Number.isSafeInteger(size) || size > BrowserHost.fileBytes || size < 100)
        throw new Error("Invalid import size");
      installed.importBegin(size);
      for (let offset = 0; offset < size; offset += BrowserHost.chunkBytes) {
        const end = Math.min(size, offset + BrowserHost.chunkBytes);
        const response = await fetch(url, { headers: { Range: `bytes=${offset}-${end - 1}` } });
        if (
          response.status !== 206 ||
          response.headers.get("Content-Range") !== `bytes ${offset}-${end - 1}/${size}`
        )
          throw new Error("Snapshot transfer was interrupted");
        const bytes = new Uint8Array(await response.arrayBuffer());
        if (bytes.length !== end - offset) throw new Error("Incomplete snapshot transfer");
        installed.importWrite(bytes);
      }
      installed.importFinish();
    }
  },
  openCopy: (copy) => copy.open(),
  async afterOpen(request) {
    if (request.source)
      await fetch(`/__import/${encodeURIComponent(request.source)}`, { method: "DELETE" });
  },
});
onmessage = owner.handle;
