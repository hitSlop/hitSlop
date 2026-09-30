import { access, constants } from "node:fs/promises";
import { join, resolve } from "node:path";
import { homedir } from "node:os";
import { coreBuildId } from "./core";

async function executable(path: string): Promise<boolean> {
  try {
    await access(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

async function nativeOverride(): Promise<string | undefined> {
  const value = process.env.HITSLOP_NATIVE_CLI;
  if (value === undefined) return;
  const path = resolve(value);
  if (!value || !(await executable(path)))
    throw new Error(`HITSLOP_NATIVE_CLI is not executable: ${value}`);
  return path;
}

export async function findNative(): Promise<string> {
  const override = await nativeOverride();
  if (override) return override;
  const candidates = [
    "/Applications/hitSlop.app/Contents/Helpers/hitslop-native",
    join(homedir(), "Applications/hitSlop.app/Contents/Helpers/hitslop-native"),
  ];
  for (const path of candidates) if (await executable(path)) return path;
  throw new Error(
    "Install hitSlop.app in /Applications or ~/Applications to use native document commands",
  );
}

export async function runNative(args: string[]) {
  const binary = await findNative();
  const identity = Bun.spawn([binary, "--core-build"], {
    stdin: "ignore", stdout: "pipe", stderr: "inherit",
  });
  const [id, status] = await Promise.all([
    new Response(identity.stdout).text(), identity.exited,
  ]);
  if (status) process.exit(status);
  if (!id.trim() || id.trim() !== await coreBuildId()) {
    throw new Error("hitSlop.app and @hitslop/cli embed different document cores; install matching versions");
  }
  const child = Bun.spawn([binary, ...args], {
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });
  const code = await child.exited;
  if (code) process.exit(code);
}
