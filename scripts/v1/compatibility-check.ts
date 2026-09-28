import { builtTemplates, discoverTemplates } from "./templates";
import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { Check } from "typebox/value";
import { RuntimeRequirementsSchema } from "../../packages/schema/src/runtime";
import { catalog, digest, releases, repository, runtimeDestinations } from "./runtime-artifacts";

export async function checkCompatibility(
  runtimeRoot = runtimeDestinations[0]!,
  checkTemplates = true,
) {
  const installed = await catalog(runtimeRoot);
  const root = join(repository, "tests/compatibility");
  const covered = new Set<number>();
  for (const name of await readdir(root)) {
    const fixture = Bun.file(join(root, name, "fixture.json"));
    if (!(await fixture.exists())) continue;
    const record = await fixture.json();
    const document = join(root, name, "document");
    if ((await digest(document)) !== record.sha256)
      throw new Error(`Preserved fixture changed: ${name}`);
    const requirements = await Bun.file(join(document, "assets/runtime.json")).json();
    if (
      !Check(RuntimeRequirementsSchema, requirements) ||
      requirements.runtimeContract !== record.runtimeContract ||
      requirements.minRuntimeRevision !== record.runtimeRevision
    )
      throw new Error(`Invalid fixture identity: ${name}`);
    const runtime = installed[String(record.runtimeContract)];
    if (!runtime || runtime.identity.runtimeRevision < record.runtimeRevision)
      throw new Error(`Unsupported fixture: ${name}`);
    covered.add(record.runtimeContract);
  }
  for (const contract of Object.keys(installed))
    if (!covered.has(Number(contract)))
      throw new Error(`Missing conformance fixture for contract ${contract}`);
  for (const release of await releases()) {
    const current = installed[String(release.runtimeContract)];
    if (current?.identity.runtimeRevision === release.runtimeRevision) continue;
    const historical = join(
      repository,
      "generated/v1/runtime-releases",
      `${release.runtimeContract}-${release.runtimeRevision}`,
      String(release.runtimeContract),
    );
    if ((await digest(historical)) !== release.sha256)
      throw new Error(
        `Restore the immutable runtime release ${release.runtimeContract}-${release.runtimeRevision} for compatibility testing`,
      );
  }
  if (!checkTemplates) return;
  const inventory = await builtTemplates();
  const packages = inventory.templates.map(({ slug }) =>
    join(repository, "generated/v1/templates", slug + ".slop"),
  );
  for (const { source } of await discoverTemplates()) {
    const dist = join(source, "dist");
    const entries = await readdir(dist, { withFileTypes: true }).catch((error) => {
      if (error.code === "ENOENT") return [];
      throw error;
    });
    for (const entry of entries)
      if (entry.isDirectory() && entry.name.endsWith(".slop"))
        packages.push(join(dist, entry.name));
  }
  for (const directory of packages) {
    // Every inventory entry is required, even if its manifest or requirements disappeared.
    await Bun.file(join(directory, "manifest.json")).json();
    const requirements = await Bun.file(join(directory, "assets/runtime.json")).json();
    if (!Check(RuntimeRequirementsSchema, requirements))
      throw new Error(`Invalid template requirements: ${directory}`);
    const runtime = installed[String(requirements.runtimeContract)];
    if (!runtime || runtime.identity.runtimeRevision < requirements.minRuntimeRevision)
      throw new Error(`Unsupported template: ${directory}`);
  }
}

if (import.meta.main) {
  if (process.argv.includes("--sealed-templates")) await checkTemplatesSealed();
  else await checkCompatibility();
}

/**
 * Release gate: every bundled template build must already be a sealed specimen, so
 * documents created from a shipped template are replayed by every later release.
 */
export async function checkTemplatesSealed() {
  const sealed = new Set<string>();
  const root = join(repository, "tests/compatibility");
  for (const name of await readdir(root)) {
    const fixture = Bun.file(join(root, name, "fixture.json"));
    if (await fixture.exists()) sealed.add((await fixture.json()).sourceSha256);
  }
  const missing: string[] = [];
  for (const { slug, bundled } of (await builtTemplates()).templates)
    if (bundled && !sealed.has(await digest(join(repository, "generated/v1/templates", slug + ".slop"))))
      missing.push(slug);
  if (missing.length)
    throw new Error(
      `Unsealed bundled templates: ${missing.join(", ")}. Seal the runtime, then run bun run fixtures:seal --write`,
    );
}
