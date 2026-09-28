import { checkHistory } from "./compatibility-history";
import { createHash } from "node:crypto";
import { lstat, readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { Check } from "typebox/value";
import { RuntimeIdentitySchema } from "../../packages/schema/src/runtime";
import identity from "../../packages/document/src/runtime-identity.json";
import { protocolVersion } from "../../packages/schema/src/bridge";

export const repository = resolve(import.meta.dir, "../..");
export const runtimeDestinations = [
  join(repository, "packages/cli/runtimes"),
  join(repository, "apps/apple/Packages/HitSlopApple/Sources/HitSlopWasm/Resources/runtimes"),
];
export async function digest(root: string): Promise<string> {
  const hash = createHash("sha256");
  async function visit(prefix: string) {
    for (const name of (await readdir(join(root, prefix))).sort()) {
      const relative = join(prefix, name),
        path = join(root, relative),
        info = await lstat(path);
      if (info.isSymbolicLink() || (!info.isDirectory() && !info.isFile()))
        throw new Error(`Invalid runtime resource: ${path}`);
      hash.update(JSON.stringify([relative, info.isDirectory() ? "directory" : "file"]));
      if (info.isDirectory()) await visit(relative);
      else
        hash.update(
          createHash("sha256")
            .update(await readFile(path))
            .digest(),
        );
    }
  }
  await visit("");
  return hash.digest("hex");
}
export async function catalog(root: string) {
  const result: Record<string, { identity: typeof identity; sha256: string }> = {};
  for (const name of (await readdir(root)).sort()) {
    if (!/^[1-9][0-9]*$/.test(name)) throw new Error(`Invalid runtime directory: ${name}`);
    const directory = join(root, name);
    if (!(await lstat(directory)).isDirectory())
      throw new Error(`Invalid runtime directory: ${directory}`);
    const value = JSON.parse(await readFile(join(directory, "identity.json"), "utf8"));
    if (!Check(RuntimeIdentitySchema, value) || String(value.runtimeContract) !== name)
      throw new Error(`Invalid runtime identity: ${directory}`);
    for (const file of ["index.js", "boot.js", "headless.js", "loro/index.js", "loro/loro_wasm_bg.wasm"])
      if (!(await lstat(join(directory, file))).isFile())
        throw new Error(`Missing runtime file: ${file}`);
    result[name] = { identity: value, sha256: await digest(directory) };
  }
  if (!Object.keys(result).length) throw new Error(`Empty runtime catalog: ${root}`);
  return result;
}
export async function verifyCopies(roots: string[]) {
  const values = await Promise.all(roots.map(catalog));
  if (values.some((value) => JSON.stringify(value) !== JSON.stringify(values[0])))
    throw new Error("Runtime contract sets, identities or bytes differ between consumers");
  return values[0]!;
}
export function verifyCurrentRuntime(values: Awaited<ReturnType<typeof catalog>>) {
  const current = values[String(identity.runtimeContract)];
  if (
    !current ||
    Object.entries(identity).some(
      ([key, value]) => current.identity[key as keyof typeof identity] !== value,
    ) ||
    Object.keys(values).some((contract) => Number(contract) > identity.runtimeContract)
  )
    throw new Error("Generated runtime identity is stale; run bun run build");
}
export async function verifyProvenance() {
  const loro = await Bun.file(Bun.resolveSync("loro-crdt/package.json", repository)).json();
  const sdk = await Bun.file(join(repository, "packages/document/package.json")).json();
  const sdkLoro = await Bun.file(
    Bun.resolveSync("loro-crdt/package.json", join(repository, "packages/document")),
  ).json();
  if (
    !Check(RuntimeIdentitySchema, identity) ||
    identity.sdkVersion !== sdk.version ||
    identity.loroVersion !== loro.version ||
    identity.loroVersion !== sdkLoro.version ||
    identity.protocolVersion !== protocolVersion
  )
    throw new Error("Runtime provenance differs from the resolved SDK/Loro/bridge");
}
export type Release = { runtimeContract: number; runtimeRevision: number; sha256: string };
export async function releases(): Promise<Release[]> {
  const values: Release[] = JSON.parse(
    await readFile(join(repository, "runtimes/releases.json"), "utf8"),
  );
  const seen = new Set<string>();
  for (const value of values) {
    const key = `${value.runtimeContract}-${value.runtimeRevision}`;
    if (
      !Number.isSafeInteger(value.runtimeContract) ||
      value.runtimeContract < 1 ||
      !Number.isSafeInteger(value.runtimeRevision) ||
      value.runtimeRevision < 1 ||
      !/^[a-f0-9]{64}$/.test(value.sha256) ||
      seen.has(key)
    )
      throw new Error("Invalid runtime release ledger");
    seen.add(key);
  }
  await checkHistory();
  return values;
}
export async function verifyReleasedIdentities(
  values: Awaited<ReturnType<typeof catalog>>,
  published?: Release[],
) {
  published ??= await releases();
  for (const release of published) {
    const installed = values[String(release.runtimeContract)];
    if (!installed) throw new Error(`Released contract ${release.runtimeContract} was removed`);
    if (installed.identity.runtimeRevision < release.runtimeRevision)
      throw new Error(`Runtime revision moved backwards: ${release.runtimeContract}`);
    if (
      installed.identity.runtimeRevision === release.runtimeRevision &&
      installed.sha256 !== release.sha256
    )
      throw new Error(
        `Released runtime ${release.runtimeContract}/${release.runtimeRevision} changed; increment runtimeRevision`,
      );
  }
}
