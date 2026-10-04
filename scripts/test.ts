import { buildCoreWasm, buildEngine } from "./core-build";
import { buildShell } from "./runtime";
import { useTestRegistry } from "./runtime-artifacts";
useTestRegistry();
const native = process.argv.includes("--native");
if (!native) {
  await buildCoreWasm();
  await buildShell();
}
await buildEngine();
if (native) await (await import("./native-fixtures")).prepareNativeFixtures();
// Package tests, and the examples' behavior tests.
const files = ["packages/{document,shell,cli,schema}/tests/**/*.test.ts", "tests/examples/**/*.test.ts"]
  .flatMap((pattern) => [...new Bun.Glob(pattern).scanSync(".")])
  .filter((file) => file.endsWith(".native.test.ts") === native)
  .sort();
const child = Bun.spawn([process.execPath, "test", ...files.map((f) => "./" + f)], {
  env: native
    ? {
        ...process.env,
        HITSLOP_NATIVE_CLI:
          process.env.HITSLOP_NATIVE_CLI ??
          `${process.cwd()}/apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native`,
      }
    : process.env,
  stdout: "inherit",
  stderr: "inherit",
});
const code = await child.exited;
if (code) process.exit(code);
