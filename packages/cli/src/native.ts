import { access, constants } from "node:fs/promises";
import { join, resolve } from "node:path";
import { homedir } from "node:os";
import { HelperProtocol } from "@hitslop/schema/constants";
import { parseHelperProtocol } from "@hitslop/schema/helper";

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

/** Refuses a helper that no longer serves, or does not yet serve, this CLI's protocol.
 * Any compatible app build works: app updates keep serving older protocols. */
export function checkProtocol(reported: string) {
  let served: { version?: unknown; minimum?: unknown } | undefined;
  try {
    served = JSON.parse(reported);
  } catch {}
  const { version, minimum } = parseHelperProtocol(served);
  if (HelperProtocol.version < minimum)
    throw new Error("hitSlop.app no longer supports this @hitslop/cli; update @hitslop/cli");
  if (HelperProtocol.version > version)
    throw new Error("This @hitslop/cli needs a newer hitSlop.app; update hitSlop");
}

export async function runNative(args: string[]) {
  const binary = await findNative();
  const probe = Bun.spawn([binary, "--protocol"], {
    stdin: "ignore", stdout: "pipe", stderr: "inherit",
  });
  const [reported, status] = await Promise.all([
    new Response(probe.stdout).text(), probe.exited,
  ]);
  if (status) process.exit(status);
  checkProtocol(reported);
  const child = Bun.spawn([binary, "--client-protocol", String(HelperProtocol.version), ...args], {
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });
  const code = await child.exited;
  if (code) process.exit(code);
}
