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
  let release = await findRelease(tag, command);
  if (!release) {
    await command(["gh", "release", "create", tag, "--draft", "--verify-tag", "--title", `hitSlop ${tag.slice(1)}`, "--notes-file", notesFile]);
    release = await findRelease(tag, command);
  }
  if (!release) throw new Error("Draft was not created");
  return release;
}
