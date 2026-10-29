import { expect, test } from "bun:test";
import {
  verifyArtifactCapabilities,
  verifyCapabilityPermissions,
} from "../../scripts/release/artifact-capabilities";
import type { exec } from "../../packages/tinys/src/cli/process";

const info = {
  NSCameraUsageDescription: "Use a camera",
  NSMicrophoneUsageDescription: "Record audio",
  NSSpeechRecognitionUsageDescription: "Transcribe audio",
};
const entitlements = {
  "com.apple.security.device.camera": true,
  "com.apple.security.device.audio-input": true,
};

function entitlementCommands(
  source: Record<string, unknown>,
  signed?: Record<string, unknown>,
): typeof exec {
  return async (command, options) => {
    if (command[0] === "/usr/bin/codesign")
      return signed
        ? { stdout: JSON.stringify(signed), stderr: "", code: 0 }
        : { stdout: "", stderr: "code object is not signed at all", code: 1 };
    // The fixture models plutil's source-file read and empty-input refusal.
    if (command.at(-1) === "-" && !options?.stdin)
      return { stdout: "", stderr: "Unexpected character at line 1", code: 1 };
    return {
      stdout: options?.stdin ? String(options.stdin) : JSON.stringify(source),
      stderr: "",
      code: 0,
    };
  };
}

test("unsigned development verification accepts absent signed entitlements", async () => {
  await verifyArtifactCapabilities(info, "fixture.app", true, entitlementCommands(entitlements));
});

test.each(Object.keys(entitlements))(
  "unsigned verification names the missing source entitlement %s",
  async (key) => {
    const source: Record<string, unknown> = { ...entitlements };
    delete source[key];
    await expect(
      verifyArtifactCapabilities(info, "fixture.app", true, entitlementCommands(source)),
    ).rejects.toThrow(key);
  },
);

test("signed verification explains empty codesign output", async () => {
  await expect(
    verifyArtifactCapabilities(info, "fixture.app", false, entitlementCommands(entitlements)),
  ).rejects.toThrow("The app is not signed; pass --unsigned only for development builds");
});

test("unsigned verification still enforces usage descriptions", async () => {
  await expect(
    verifyArtifactCapabilities({}, "fixture.app", true, entitlementCommands(entitlements)),
  ).rejects.toThrow("NSCameraUsageDescription");
});

test("signed verification accepts supported capabilities", async () => {
  await verifyArtifactCapabilities(
    info,
    "fixture.app",
    false,
    entitlementCommands({}, entitlements),
  );
});

test.each(Object.keys(entitlements))(
  "signed verification names the missing signed entitlement %s",
  async (key) => {
    const signed: Record<string, unknown> = { ...entitlements };
    delete signed[key];
    await expect(
      verifyArtifactCapabilities(
        info,
        "fixture.app",
        false,
        entitlementCommands(entitlements, signed),
      ),
    ).rejects.toThrow(key);
  },
);

test("release privacy declarations admit all supported capabilities", () => {
  expect(() => verifyCapabilityPermissions(info, entitlements)).not.toThrow();
});

test.each(Object.keys(info))("release privacy check names missing or empty %s", (key) => {
  for (const value of [undefined, "", "  ", true]) {
    const fixture: Record<string, unknown> = { ...info, [key]: value };
    if (value === undefined) delete fixture[key];
    expect(() => verifyCapabilityPermissions(fixture, entitlements)).toThrow(key);
  }
});

test.each(Object.keys(entitlements))(
  "release privacy check names missing or disabled %s",
  (key) => {
    for (const value of [undefined, false, "true"]) {
      const fixture: Record<string, unknown> = { ...entitlements, [key]: value };
      if (value === undefined) delete fixture[key];
      expect(() => verifyCapabilityPermissions(info, fixture)).toThrow(key);
    }
  },
);


test("empty compatibility history only permits unsigned pre-release app acceptance", async () => {
  const { verifyAppCorpus } = await import("../../scripts/release/app-commands");
  const { mkdtemp, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const root = await mkdtemp(join(tmpdir(), "tinys-app-bootstrap-"));
  const keys = ["TINYS_COMPAT_ROOT", "TINYS_COMPAT_BOOTSTRAP", "TINYS_RELEASE_TAG"] as const;
  const previous = Object.fromEntries(keys.map(key => [key, process.env[key]]));
  try {
    process.env.TINYS_COMPAT_ROOT = root;
    process.env.TINYS_COMPAT_BOOTSTRAP = "1";
    delete process.env.TINYS_RELEASE_TAG;
    await expect(verifyAppCorpus("unused", { allowUnreleased: true })).resolves.toBeUndefined();
    await expect(verifyAppCorpus("unused")).rejects.toThrow("No corpus");
    process.env.TINYS_RELEASE_TAG = "v1.0.0";
    await expect(verifyAppCorpus("unused", { allowUnreleased: true })).rejects.toThrow("No corpus");
  } finally {
    for (const key of keys) {
      if (previous[key] === undefined) delete process.env[key];
      else process.env[key] = previous[key];
    }
    await rm(root, { recursive: true, force: true });
  }
});
