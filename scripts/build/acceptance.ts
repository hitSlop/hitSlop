/** Released acceptance is data, independent of today's authoring rules. Until the first
 * release names a marker its record follows authoring; afterwards it is immutable. */
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { repository } from "../lib/artifacts";
import { SlopManifestSchema } from "../../packages/hitslop/src/schema/manifest";
import { CommandMetadata } from "../../packages/hitslop/src/schema/commands";
import { AppLimits, AssetLimits, AttachmentLimits, DefaultWindowRadius, PackageFormat, ShapeLimits,
  StorageLimits, ThemeLimit, ThemeFileLimit, ThemeTokenRule } from "../../packages/hitslop/src/schema/constants";

export const StorageVersion = 1;
export const currentAppAcceptance = { manifest: SlopManifestSchema, commands: CommandMetadata,
  limits: { AppLimits, AssetLimits, DefaultWindowRadius, ShapeLimits, ThemeLimit, ThemeFileLimit, ThemeTokenRule } };
export const currentStorageAcceptance = { StorageLimits, AttachmentLimits };
export type AppAcceptance = typeof currentAppAcceptance;
export type StorageAcceptance = typeof currentStorageAcceptance;

export async function acceptanceContracts(root = repository) {
  const outputs: Record<string, string> = {};
  const records = await Promise.all((await readdir(join(root, "tests/compat"))).map(async name => {
    const file = join(root, "tests/compat", name, "release.json");
    return JSON.parse(await readFile(file, "utf8").catch(() => "null"));
  }));
  async function record<T>(kind: "packageFormat" | "storage", version: number, value: T): Promise<T> {
    const path = `packages/hitslop/acceptance/${kind}-${version}.json`;
    const frozen = records.filter(record => record?.frozen && record.markers[kind] === version);
    let text: string;
    if (frozen.length) {
      text = await readFile(join(root, path), "utf8");
      const digest = createHash("sha256").update(text).digest("hex");
      for (const release of frozen)
        if (release.acceptance?.[path] !== digest) throw new Error(`Released acceptance changed or is missing: ${path} (${release.release})`);
    } else text = JSON.stringify(value, null, 2) + "\n";
    outputs[path] = text;
    return JSON.parse(text) as T;
  }
  const app = await record("packageFormat", PackageFormat, currentAppAcceptance);
  const storage = await record("storage", StorageVersion, currentStorageAcceptance);
  outputs[`packages/hitslop/generated/manifest-format-${PackageFormat}.schema.json`] = JSON.stringify(app.manifest, null, 2) + "\n";
  outputs[`packages/hitslop/generated/commands-format-${PackageFormat}.schema.json`] = JSON.stringify(app.commands, null, 2) + "\n";
  return { app, storage, outputs };
}
