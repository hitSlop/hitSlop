import { strict as assert } from "node:assert";
import { fileURLToPath } from "node:url";
import { exec } from "../../packages/tinys/src/cli/process";
import type { Capability } from "../../packages/tinys/src/wire/app.generated";

const capabilityPermissions = {
  camera: {
    usage: "NSCameraUsageDescription",
    entitlements: ["com.apple.security.device.camera"],
  },
  microphone: {
    usage: "NSMicrophoneUsageDescription",
    entitlements: ["com.apple.security.device.audio-input"],
  },
  speech: {
    usage: "NSSpeechRecognitionUsageDescription",
    entitlements: [],
  },
} satisfies Record<Capability, { usage: string | null; entitlements: readonly string[] }>;

/** Check the packaged privacy declarations and signing entitlements. */
export function verifyCapabilityPermissions(
  info: Readonly<Record<string, unknown>>,
  entitlements: Readonly<Record<string, unknown>>,
): void {
  for (const [capability, permission] of Object.entries(capabilityPermissions)) {
    if (permission.usage !== null) {
      const description = info[permission.usage];
      assert.ok(
        typeof description === "string" && description.trim().length > 0,
        `Missing or empty ${permission.usage} for ${capability}`,
      );
    }
    for (const key of permission.entitlements)
      assert.equal(entitlements[key], true, `Missing or disabled ${key} for ${capability}`);
  }
}

/** Development builds check the signing source; releases must carry signed entitlements. */
export async function verifyArtifactCapabilities(
  info: Readonly<Record<string, unknown>>,
  app: string,
  unsigned: boolean,
  execute: typeof exec = exec,
): Promise<void> {
  const convert = ["/usr/bin/plutil", "-convert", "json", "-o", "-"];
  const source = fileURLToPath(
    new URL("../../apps/apple/App/macOS/tinys.entitlements", import.meta.url),
  );
  const signed = unsigned
    ? undefined
    : await execute(["/usr/bin/codesign", "--display", "--entitlements", "-", "--xml", app]);
  if (signed) {
    assert.ok(
      signed.stdout.trim().length > 0,
      "The app is not signed; pass --unsigned only for development builds",
    );
    if (signed.code) throw new Error(signed.stderr.trim() || "Cannot read signed entitlements");
  }
  const result = await execute(
    [...convert, unsigned ? source : "-"],
    signed ? { stdin: signed.stdout } : {},
  );
  if (result.code) throw new Error(result.stderr.trim() || "Cannot read app entitlements");
  verifyCapabilityPermissions(info, JSON.parse(result.stdout));
}
