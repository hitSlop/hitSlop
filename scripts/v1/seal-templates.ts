/** Explicit release preparation only. Ordinary checks never create or rewrite fixtures. */
import { cp, mkdir, mkdtemp, readdir, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { builtTemplates } from "./templates";
import {
  digest,
  templateAppDigest,
  releases,
  runtimeDestinations,
  verifyCopies,
  verifyCurrentRuntime,
  verifyReleasedIdentities,
} from "./runtime-artifacts";
import { runRuntime } from "./compatibility";
import { checkHistory } from "./compatibility-history";

await checkHistory();
const runtimes = await verifyCopies(runtimeDestinations);
verifyCurrentRuntime(runtimes);
await verifyReleasedIdentities(runtimes);
const published = await releases();
const corpus = "tests/compatibility";
const existing = new Set<string>();
for (const name of await readdir(corpus)) {
  const path = Bun.file(join(corpus, name, "fixture.json"));
  if (await path.exists()) {
    const record = await path.json();
    if (record.kind === "template") {
      const document = join(corpus, name, "document");
      if (await digest(document) !== record.sha256)
        throw new Error(`Preserved fixture changed: ${name}`);
      existing.add(await templateAppDigest(document));
    }
  }
}
for (const { slug, bundled } of (await builtTemplates()).templates) {
  if (!bundled) continue;
  const source = join("generated/v1/templates", `${slug}.slop`);
  if (existing.has(await templateAppDigest(source))) continue;
  const sourceSha256 = await digest(source);
  const requirements = await Bun.file(join(source, "assets/runtime.json")).json();
  const selected = runtimes[String(requirements.runtimeContract)];
  if (
    !selected ||
    !published.some(
      (r) =>
        r.runtimeContract === requirements.runtimeContract &&
        r.runtimeRevision === selected.identity.runtimeRevision &&
        r.sha256 === selected.sha256,
    )
  )
    throw new Error(`Seal the tested runtime before freezing template ${slug}`);
  const name = `template-${slug}-${sourceSha256.slice(0, 16)}`;
  if (!process.argv.includes("--write")) {
    console.log(`Would preserve ${name}`);
    continue;
  }
  const stage = await mkdtemp(join(corpus, ".sealing-"));
  try {
    const document = join(stage, "document");
    await cp(source, document, { recursive: true });
    const result = await runRuntime(
      join(runtimeDestinations[0]!, String(requirements.runtimeContract)),
      document,
    );
    await writeFile(join(stage, "expected.json"), JSON.stringify(result.state, null, 2) + "\n");
    await writeFile(
      join(stage, "fixture.json"),
      JSON.stringify(
        {
          kind: "template",
          runtimeContract: requirements.runtimeContract,
          runtimeRevision: requirements.minRuntimeRevision,
          sourceSha256,
          runtimeSha256: selected.sha256,
          sha256: await digest(document),
        },
        null,
        2,
      ) + "\n",
    );
    // mkdir is exclusive: even an interrupted or colliding specimen is never replaced.
    const destination = join(corpus, name);
    await mkdir(destination);
    for (const entry of await readdir(stage))
      await rename(join(stage, entry), join(destination, entry));
    console.log(`Preserved ${name}`);
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}
