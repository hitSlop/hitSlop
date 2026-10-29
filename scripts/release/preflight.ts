import { readFile, appendFile } from "node:fs/promises";
import { run, exec } from "../../packages/tinys/src/cli/process";
import { assetBytes, releaseInventory, releaseRepository, stableTag, requiredAsset } from "../lib/release-assets";
import { advancesBuild, preservesLatest, validatePublication, validateRecord, type ReleaseRecord } from "./record";

export async function projectVersion() {
  const project = await readFile("apps/apple/project.yml", "utf8");
  const version = project.match(/MARKETING_VERSION: "([^"]+)"/)?.[1];
  const build = project.match(/CURRENT_PROJECT_VERSION: "([^"]+)"/)?.[1];
  if (!version || !build || !stableTag.test(`tinys-v${version}`)) throw new Error("Invalid release version");
  const pkg = await Bun.file("packages/tinys/package.json").json();
  const workspace = await Bun.file("package.json").json();
  if (pkg.version !== version || workspace.version !== version) throw new Error("Mac, npm and workspace versions must agree");
  return { version, build, tag: `tinys-v${version}` };
}

export function validatePublicationSettings(environment: { protection_rules: { type: string; reviewers?: unknown[] }[] }) {
  if (!environment.protection_rules.some(rule => rule.type === "required_reviewers" && rule.reviewers?.length))
    throw new Error("The release environment must require manual acceptance");
}

export function validateCandidateAttempt(attempt: string | undefined) {
  if (attempt && attempt !== "1") throw new Error("Start a new workflow dispatch to rebuild a candidate; only publication may be retried");
}

export function checkReleaseTags(tags: string[], releases: { tag_name: string }[], candidate: string, resuming = false) {
  if (!resuming && tags.includes(candidate)) throw new Error(`${candidate} already exists; resume its publication or recover the unpublished tag before building`);
  return tags.filter(tag => stableTag.test(tag) && tag !== candidate && !releases.some(release => release.tag_name === tag));
}

/** Publication is serialized. Drafts and incomplete prior publications
 * must be recovered before a new version starts. Ordinary corpus replay ignores drafts. */
export async function checkReleaseHistory(version: string, build: string, own?: ReleaseRecord, source: {
  list?: typeof releaseInventory; tags?: () => Promise<string[]>;
} = {}) {
  const all = await (source.list ?? releaseInventory)();
  const tags = source.tags ? await source.tags() : (JSON.parse(await run(["gh", "api", "--paginate", "--slurp", `repos/${releaseRepository}/tags?per_page=100`])) as { name: string }[][]).flat().map(tag => tag.name);
  const stray = checkReleaseTags(tags, all, `tinys-v${version}`, !!own);
  if (stray.length) console.warn(`Unpublished tags (not release history): ${stray.join(", ")}`);
  let latest: ReleaseRecord | undefined;
  for (const release of all) {
    if (own?.tag === release.tag_name) continue;
    if (release.draft || release.prerelease) throw new Error(`Incomplete publication ${release.tag_name}; recover it first`);
    if (!release.immutable) throw new Error(`${release.tag_name} is not immutable`);
    const record: ReleaseRecord = JSON.parse((await assetBytes(requiredAsset(release, "release-record.json").id)).toString());
    validateRecord(record, release.tag_name, record.commit);
    if (record.macVersion === version || !preservesLatest(version, record.macVersion) || !advancesBuild(build, record.macBuild))
      throw new Error(`Candidate version and Mac build must advance ${release.tag_name} (build ${record.macBuild})`);
    if (!latest || advancesBuild(record.macBuild, latest.macBuild)) latest = record;
  }
  if (latest && !own) {
    // Remote outputs, rather than the lifetime of an Actions run, establish completion.
    // This also permits manual recovery of the original artifacts after reruns expire.
    const npm = await fetch("https://registry.npmjs.org/@hitslop%2ftinys/latest", { cache: "no-store" });
    if (!npm.ok) throw new Error(`Incomplete publication ${latest.tag}: npm unavailable`);
    const github = JSON.parse(await run(["gh", "api", `repos/${releaseRepository}/releases/latest`]));
    validatePublication(latest, { npm: await npm.json() as Parameters<typeof validatePublication>[1]["npm"],
      githubTag: github.tag_name });
  }
  return latest;
}

if (import.meta.main) {
  validateCandidateAttempt(process.env.GITHUB_RUN_ATTEMPT);
  const environment = await run(["gh", "api", `repos/${releaseRepository}/environments/release`]);
  validatePublicationSettings(JSON.parse(environment));
  const { version, build, tag } = await projectVersion();
  const latest = await checkReleaseHistory(version, build);
  const branch = process.env.GITHUB_REF_NAME;
  if (branch && branch !== "master") {
    if (!branch.startsWith("hotfix/") || !latest) throw new Error("Release candidates must run on master or a hotfix from the latest release");
    const ancestor = await exec(["git", "merge-base", "--is-ancestor", `${latest.tag}^{commit}`, "HEAD"]);
    if (ancestor.code) throw new Error("Hotfix branch must contain the latest release");
  }
  if ((await run(["git", "status", "--porcelain"])).trim()) throw new Error("Release candidate source tree must be clean");
  if (process.env.GITHUB_OUTPUT) await appendFile(process.env.GITHUB_OUTPUT, `tag=${tag}\nversion=${version}\n`);
  console.log(`Ready to build ${tag}, Mac build ${build}`);
}
