import { run } from "../../packages/hitslop/src/cli/process";
import { releaseRepository, type PublishedRelease } from "../lib/release-assets";

/** The tag endpoint only returns published releases. Authenticated listing also
 * finds drafts; read the selected ID again for its current assets and lock state. */
export async function findRelease(tag: string, command = run): Promise<PublishedRelease | undefined> {
  const pages: PublishedRelease[][] = JSON.parse(await command(["gh", "api", "--paginate", "--slurp", `repos/${releaseRepository}/releases?per_page=100`]));
  const matches = pages.flat().filter(release => release.tag_name === tag);
  if (matches.length > 1) throw new Error(`Multiple releases for ${tag}`);
  const match = matches[0];
  if (!match) return undefined;
  const release: PublishedRelease = JSON.parse(await command(["gh", "api", `repos/${releaseRepository}/releases/${match.id}`]));
  if (release.id !== match.id || release.tag_name !== tag) throw new Error(`Release identity changed for ${tag}`);
  return release;
}

/** Resume the same draft after an interrupted upload. */
export async function ensureDraftRelease(tag: string, notesFile: string, command = run): Promise<PublishedRelease> {
  const release = await findRelease(tag, command);
  if (release) return release;
  // Keep --verify-tag's refusal to create an unverified tag. The POST response owns
  // the new draft's identity; listing it immediately can miss a successful creation.
  await command(["gh", "api", `repos/${releaseRepository}/git/ref/tags/${tag}`]);
  const created: PublishedRelease = JSON.parse(await command([
    "gh", "api", "--method", "POST", `repos/${releaseRepository}/releases`,
    "-f", `tag_name=${tag}`, "-f", `name=hitSlop ${tag.slice(1)}`,
    "-F", "draft=true", "-F", `body=@${notesFile}`,
  ]));
  if (!Number.isSafeInteger(created.id) || created.id <= 0 || created.tag_name !== tag || !created.draft)
    throw new Error("Created release identity differs from the requested draft");
  return created;
}
