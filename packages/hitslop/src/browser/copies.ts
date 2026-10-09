import { BrowserHost } from "../schema/constants";
export type Copy = { id: string; title: string; created: number; state: "importing" | "ready" | "missing" };
let database: Promise<IDBDatabase> | undefined;
function openDatabase() { return database ??= new Promise<IDBDatabase>((resolve, reject) => {
  const open = indexedDB.open("hitslop-browser", 1);
  open.onupgradeneeded = () => open.result.createObjectStore("copies", { keyPath: "id" });
  open.onsuccess = () => resolve(open.result);
  open.onerror = () => reject(open.error);
}); }
async function transaction<T>(mode: IDBTransactionMode, operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await openDatabase();
  return new Promise((resolve, reject) => {
    const tx = db.transaction("copies", mode);
    const request = operation(tx.objectStore("copies"));
    tx.oncomplete = () => resolve(request.result);
    tx.onerror = tx.onabort = () => reject(tx.error ?? request.error);
  });
}
export const listCopies = () => transaction<Copy[]>("readonly", store => store.getAll());
export const putCopy = (copy: Copy) => transaction("readwrite", store => store.put(copy));
const removeRecord = (id: string) => transaction("readwrite", store => store.delete(id));
export const lockName = (id: string) => `hitslop-copy:${id}`;
export async function removeCopy(id: string) {
  return navigator.locks.request(lockName(id), { ifAvailable: true }, async lock => {
    if (!lock) throw new Error("This copy is open in another tab. Close it before deleting.");
    await checkContainer(id);
    await removeFiles(id);
    await removeRecord(id);
  });
}
/** Check before installing sahpool or deleting its files. Pre-launch unmarked pools are unsupported. */
export async function checkContainer(id: string, create = false) {
  const root = await navigator.storage.getDirectory();
  const copies = await root.getDirectoryHandle("copies", { create: true });
  let folder: FileSystemDirectoryHandle;
  try { folder = await copies.getDirectoryHandle(id); }
  catch (error) {
    if (!create || (error as DOMException).name !== "NotFoundError") throw error;
    folder = await copies.getDirectoryHandle(id, { create: true });
    const marker = await (await folder.getFileHandle("container.json", { create: true })).createWritable();
    await marker.write(JSON.stringify({ format: BrowserHost.containerFormat })); await marker.close();
    return;
  }
  const format = await folder.getFileHandle("container.json").then(file => file.getFile()).then(file => file.text()).then(JSON.parse).catch(() => null);
  if (!format || typeof format !== "object" || !("format" in format) || format.format !== BrowserHost.containerFormat) throw new Error("Unsupported browser storage format; open with the matching hitSlop version. This copy was not changed.");
}
async function removeFiles(id: string) {
  const root = await navigator.storage.getDirectory();
  const copies = await root.getDirectoryHandle("copies", { create: true });
  await copies.removeEntry(id, { recursive: true }).catch(error => { if (error.name !== "NotFoundError") throw error; });
  const exports = await root.getDirectoryHandle("exports", { create: true });
  await exports.removeEntry(id, { recursive: true }).catch(error => { if (error.name !== "NotFoundError") throw error; });
}
/** IndexedDB contains only the catalog. Every recovery/delete takes the same lock as an
 * open owner, so listing copies never removes an active import or export. */
export async function cleanup() {
  const entries = await listCopies();
  const root = await navigator.storage.getDirectory();
  const copies = await root.getDirectoryHandle("copies", { create: true });
  const names = new Set(entries.map(entry => entry.id));
  for await (const [name] of (copies as any).entries()) names.add(name);
  for (const id of names) await navigator.locks.request(lockName(id), { ifAvailable: true }, async lock => {
    if (!lock) return;
    const entry = await transaction<Copy | undefined>("readonly", store => store.get(id));
    try { await checkContainer(id); } catch (error) {
      if ((error as DOMException).name === "NotFoundError" && entry?.state === "ready") await putCopy({ ...entry, state: "missing" });
      return;
    }
    if (!entry || entry.state === "importing") { await removeFiles(id); await removeRecord(id); }
    else {
      try { await copies.getDirectoryHandle(id); }
      catch (error) {
        if ((error as DOMException).name !== "NotFoundError") throw error;
        await putCopy({ ...entry, state: "missing" });
      }
      const exports = await root.getDirectoryHandle("exports", { create: true });
      await exports.removeEntry(id, { recursive: true }).catch(error => { if (error.name !== "NotFoundError") throw error; });
    }
  });
}
