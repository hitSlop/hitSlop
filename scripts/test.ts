import { buildCoreWasm } from "./core-build";
import { buildShell } from "./runtime";
const native = process.argv.includes("--native");
if (!native) {
  await buildCoreWasm();
  await buildShell();
}
if (native) await (await import("./native-fixtures")).prepareNativeFixtures();
const files = [...new Bun.Glob("packages/{document,shell,cli,schema}/tests/**/*.test.ts").scanSync(".")]
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
