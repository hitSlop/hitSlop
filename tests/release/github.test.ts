import { expect, test } from "bun:test";
import { run } from "../../packages/hitslop/src/cli/process";
import { releaseRepository, type PublishedRelease } from "../../scripts/lib/release-assets";
import { ensureDraftRelease, findRelease } from "../../scripts/release/github";

const tag = "v7.2.0";
const draft = (): PublishedRelease => ({ id: 42, tag_name: tag, draft: true, prerelease: false, immutable: false, assets: [] });

// Model GitHub's public tag lookup separately from its authenticated draft listing.
// The first page never holds our release, so discovery must include later pages.
function github(releases: PublishedRelease[] = []) {
  const command: typeof run = async args => {
    if (args[0] !== "gh") throw new Error("Unexpected executable");
    if (args[1] === "api") {
      const path = args.at(-1)!;
      const base = `repos/${releaseRepository}/releases`;
      if (path === `${base}?per_page=100`) {
        if (!args.includes("--paginate") || !args.includes("--slurp")) throw new Error("Listing requires all pages");
        return JSON.stringify([[{ ...draft(), id: 1, tag_name: "v9.0.0" }], releases]);
      }
      const release = path.startsWith(`${base}/tags/`)
        ? releases.find(entry => entry.tag_name === path.slice(`${base}/tags/`.length) && !entry.draft)
        : releases.find(entry => path === `${base}/${entry.id}`);
      if (release) return JSON.stringify(release);
      throw new Error("gh: Not Found (HTTP 404)");
    }
    if (args[1] === "release" && args[2] === "create") {
      if (releases.some(entry => entry.tag_name === args[3])) throw new Error("Release already exists");
      releases.push({ ...draft(), tag_name: args[3]! });
      return "created";
    }
    throw new Error(`Unexpected GitHub operation: ${args.join(" ")}`);
  };
  return { releases, command };
}

test("authenticated discovery finds a draft beyond the first page", async () => {
  const release = { ...draft(), assets: [{ id: 7, name: "retained.tgz" }] };
  expect(await findRelease(tag, github([release]).command)).toEqual(release);
});

test("published releases and absent tags are distinguished", async () => {
  const release = { ...draft(), draft: false, immutable: true };
  expect(await findRelease(tag, github([release]).command)).toEqual(release);
  expect(await findRelease(tag, github().command)).toBeUndefined();
});

test("staging discovers a newly created draft and resumes its partial upload", async () => {
  const remote = github();
  expect(await ensureDraftRelease(tag, "notes.md", remote.command)).toEqual(draft());
  remote.releases[0]!.assets.push({ id: 7, name: "retained.tgz" });
  expect(await ensureDraftRelease(tag, "notes.md", remote.command)).toEqual(remote.releases[0]!);
  expect(remote.releases).toHaveLength(1);
});

test.each([403, 404, 429, 500])("release discovery preserves an API failure (%s)", async status => {
  const command: typeof run = async () => { throw new Error(`GitHub unavailable (HTTP ${status})`); };
  await expect(findRelease(tag, command)).rejects.toThrow(`HTTP ${status}`);
});

test("ambiguous release tags are refused", async () => {
  await expect(findRelease(tag, github([draft(), { ...draft(), id: 43 }]).command)).rejects.toThrow("Multiple releases");
});
