import { builtTemplates, discoverTemplates } from "./templates";
import { readdir } from "node:fs/promises";
import { basename, join } from "node:path";
import { Check } from "typebox/value";
import { RuntimeRequirementsSchema } from "../../packages/schema/src/runtime";
import { catalog, digest, releases, repository, runtimeDestinations, templateAppDigest } from "./runtime-artifacts";

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
 * Release gate: every bundled template's authored content must have a sealed specimen, so
 * documents created from a shipped template are replayed by every later release.
 */
export async function checkTemplatesSealed() {
  const packages = (await builtTemplates()).templates
    .filter(({ bundled }) => bundled)
    .map(({ slug }) => join(repository, "generated/v1/templates", slug + ".slop"));
  await checkTemplateSpecimens(packages, join(repository, "tests/compatibility"));
}

export async function checkTemplateSpecimens(packages: string[], root: string) {
  const sealed = new Set<string>();
  for (const name of await readdir(root)) {
    const fixture = Bun.file(join(root, name, "fixture.json"));
    if (!(await fixture.exists())) continue;
    const record = await fixture.json();
    if (record.kind !== "template") continue;
    const document = join(root, name, "document");
    if (await digest(document) !== record.sha256)
      throw new Error(`Preserved fixture changed: ${name}`);
    sealed.add(await templateAppDigest(document));
  }
  const missing: string[] = [];
  for (const directory of packages)
    if (!sealed.has(await templateAppDigest(directory))) missing.push(basename(directory, ".slop"));
  if (missing.length)
    throw new Error(
      `Unsealed bundled templates: ${missing.join(", ")}. Seal the runtime, then run bun run fixtures:seal --write`,
    );
}
