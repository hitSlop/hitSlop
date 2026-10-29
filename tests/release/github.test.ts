import { expect, test } from "bun:test";
import { run } from "../../packages/tinys/src/cli/process";
import { releaseRepository, type PublishedRelease } from "../../scripts/lib/release-assets";
import { ensureDraftRelease, findRelease } from "../../scripts/release/github";

const tag = "tinys-v7.2.0";
const draft = (): PublishedRelease => ({ id: 42, tag_name: tag, draft: true, prerelease: false, immutable: false, assets: [] });

// Model GitHub's public tag lookup separately from its authenticated draft listing.
// The first page never holds our release, so discovery must include later pages.
function github(releases: PublishedRelease[] = [], hideCreated = false) {
  const hidden = new Set<number>();
  const command: typeof run = async args => {
    if (args[0] !== "gh") throw new Error("Unexpected executable");
    if (args[1] === "api") {
      const path = args.find(arg => arg.startsWith(`repos/${releaseRepository}/`))!;
      const base = `repos/${releaseRepository}/releases`;
      if (path === `repos/${releaseRepository}/git/ref/tags/${tag}`) return JSON.stringify({ ref: `refs/tags/${tag}` });
      if (path === base && args.includes("POST")) {
        const createdTag = args.find(arg => arg.startsWith("tag_name="))!.slice("tag_name=".length);
        if (releases.some(entry => entry.tag_name === createdTag)) throw new Error("Release already exists");
        const created = { ...draft(), tag_name: createdTag };
        releases.push(created);
        if (hideCreated) hidden.add(created.id);
        return JSON.stringify(created);
      }
      if (path === `${base}?per_page=100`) {
        if (!args.includes("--paginate") || !args.includes("--slurp")) throw new Error("Listing requires all pages");
        return JSON.stringify([[{ ...draft(), id: 1, tag_name: "tinys-v9.0.0" }], releases.filter(release => !hidden.has(release.id))]);
      }
      const release = path.startsWith(`${base}/tags/`)
        ? releases.find(entry => entry.tag_name === path.slice(`${base}/tags/`.length) && !entry.draft)
        : releases.find(entry => path === `${base}/${entry.id}`);
      if (release) return JSON.stringify(release);
      throw new Error("gh: Not Found (HTTP 404)");
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

test("staging keeps the created draft when the listing has not caught up", async () => {
  const remote = github([], true);
  expect(await ensureDraftRelease(tag, "notes.md", remote.command)).toEqual(draft());
  expect(remote.releases).toHaveLength(1);
  expect(await findRelease(tag, remote.command)).toBeUndefined();
});

test("staging refuses a missing tag before creating a draft", async () => {
  const remote = github();
  const command: typeof run = async args => {
    if (args.some(arg => arg.includes("/git/ref/tags/"))) throw new Error("Tag not found (HTTP 404)");
    return remote.command(args);
  };
  await expect(ensureDraftRelease(tag, "notes.md", command)).rejects.toThrow("Tag not found");
  expect(remote.releases).toHaveLength(0);
});

test("staging refuses a creation response for another release", async () => {
  const remote = github();
  const command: typeof run = async args => {
    if (args.includes("POST")) return JSON.stringify({ ...draft(), tag_name: "tinys-v9.0.0" });
    return remote.command(args);
  };
  await expect(ensureDraftRelease(tag, "notes.md", command)).rejects.toThrow("Created release identity differs");
});

test.each([403, 404, 429, 500])("release discovery preserves an API failure (%s)", async status => {
  const command: typeof run = async () => { throw new Error(`GitHub unavailable (HTTP ${status})`); };
  await expect(findRelease(tag, command)).rejects.toThrow(`HTTP ${status}`);
});

test("ambiguous release tags are refused", async () => {
  await expect(findRelease(tag, github([draft(), { ...draft(), id: 43 }]).command)).rejects.toThrow("Multiple releases");
});
