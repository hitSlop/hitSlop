// Mutates only a disposable copy. The actual core and generated wire stay intact.
import { cp, mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
const root = import.meta.dir;
const destination = join(root, "dist/sensitivity");
const evidence = resolve(root, "../../.hitslop/v1-evidence/hitslop-core");
await mkdir(destination, { recursive: true });
await mkdir(evidence, { recursive: true });
await cp(join(root, "core"), join(destination, "core"), { recursive: true });
await cp(join(root, "fixtures"), join(destination, "fixtures"), { recursive: true });
await cp(join(root, "Cargo.lock"), join(destination, "Cargo.lock"));
const manifest = (await Bun.file(join(root, "Cargo.toml")).text()).replace('members = ["core", "native", "wasm"]', 'members = ["core"]');
await Bun.write(join(destination, "Cargo.toml"), manifest);
const source = join(destination, "core/src/lib.rs");
const text = await Bun.file(source).text();
// Skip recovery of a partially applied rejected batch; its earlier intents then remain live.
const marker = "self.abort(&before)?; // Atomicity sensitivity";
if (!text.includes(marker)) throw new Error("Atomicity mutation target changed");
await Bun.write(source, text.replace(marker, "// Atomicity sensitivity: recovery removed"));
const p = Bun.spawn(["cargo", "test", "--offline", "--release", "--manifest-path", join(destination, "Cargo.toml"), "--target-dir", join(destination, "target"), "atomic_rejection", "--", "--nocapture"], {
  cwd: root, env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH}` }, stdout: "pipe", stderr: "pipe",
});
const [stdout, stderr, exit] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
await Bun.write(join(evidence, "atomic-sensitivity.log"), stdout + stderr);
if (exit === 0 || !(stdout + stderr).includes("rejected batch changed state/version/publication")) {
  throw new Error(`Atomicity test did not catch the intended fault (exit ${exit}); inspect evidence`);
}
console.log("Atomicity owner test caught partial mutation in a disposable source copy.");
