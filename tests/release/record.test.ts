import { checkReleaseHistory, checkReleaseTags, validateCandidateAttempt, validatePublicationSettings } from "../../scripts/release/preflight";
import { expect, test } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { sha256 } from "../../scripts/lib/artifacts";
import { stableTag } from "../../scripts/lib/release-assets";
import { DocumentLineage } from "../../packages/pop/src/schema/constants";
import { advancesBuild, appcastBuild, assertQualified, legacyAppcast, validateApproval, validateCandidate, validatePublication, preservesLatest, validateRecord, verifyArtifacts, type ReleaseRecord } from "../../scripts/release/record";

test("pop release tags do not include the preserved hitSlop lineage", () => {
  expect(stableTag.test("pop-v1.0.0")).toBe(true);
  expect(stableTag.test("v1.0.0")).toBe(false);
  expect(stableTag.test("pop-v01.0.0")).toBe(false);
  expect(checkReleaseTags(["v1.0.0", "v1.3.0"], [], "pop-v1.0.0")).toEqual([]);
});

test("resumed candidates require their original identity and complete, unchanged artifacts", async () => {
  const folder = await mkdtemp(join(tmpdir(), "pop-release-record-"));
  const saved: ReleaseRecord = { tag: "pop-v7.2.0", macVersion: "7.2.0", macBuild: "42", commit: "c".repeat(40), run: "123", attempt: "1", npmIntegrity: "sha512-" + "a".repeat(86) + "==", corpus: { file: "compat-corpus-7.2.0.tar.gz", sha256: sha256(Buffer.from("compat-corpus-7.2.0.tar.gz")) }, qualified: {}, artifacts: {} };
  try {
    const feeds = {
      "dotpop-appcast.xml": '<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"><channel><item><enclosure sparkle:version="42"/></item></channel></rss>',
      "appcast.xml": legacyAppcast,
    };
    for (const name of ["compat-corpus-7.2.0.tar.gz", "hitslop-pop-7.2.0.tgz", "dotpop.dmg", "dotpop.zip", ...Object.keys(feeds), "dotpop-7.2.0.md", "release-notes.md"]) {
      const bytes = feeds[name as keyof typeof feeds] ?? name;
      await writeFile(join(folder, name), bytes);
      saved.artifacts[name] = sha256(Buffer.from(bytes));
    }
    validateRecord(saved, saved.tag, saved.commit);
    await verifyArtifacts(saved, folder);
    expect(() => validateRecord(saved, saved.tag, "d".repeat(40))).toThrow("different tag or commit");
    expect(() => validateRecord({ ...saved, artifacts: {} }, saved.tag, saved.commit)).toThrow("missing");
    for (const name of ["dotpop-appcast.xml", "appcast.xml", "dotpop-7.2.0.md", "release-notes.md"]) {
      const artifacts = { ...saved.artifacts };
      delete artifacts[name];
      expect(() => validateRecord({ ...saved, artifacts }, saved.tag, saved.commit)).toThrow(`missing ${name}`);
      await writeFile(join(folder, name), "regenerated notes");
      await expect(verifyArtifacts(saved, folder)).rejects.toThrow(`Artifact differs from release record: ${name}`);
      await writeFile(join(folder, name), feeds[name as keyof typeof feeds] ?? name);
    }
    for (const [name, bytes, error] of [
      ["appcast.xml", feeds["dotpop-appcast.xml"], "Legacy appcast must remain empty"],
      ["dotpop-appcast.xml", feeds["appcast.xml"], "no Mac build"],
      ["dotpop-appcast.xml", feeds["dotpop-appcast.xml"].replace('"42"', '"41"'), "Appcast build differs"],
    ] as const) {
      await writeFile(join(folder, name), bytes);
      const artifacts = { ...saved.artifacts, [name]: sha256(Buffer.from(bytes)) };
      await expect(verifyArtifacts({ ...saved, artifacts }, folder)).rejects.toThrow(error);
      await writeFile(join(folder, name), feeds[name]);
    }
    expect(() => validateRecord({ ...saved, artifacts: { ...saved.artifacts, "../outside": "a".repeat(64) } }, saved.tag, saved.commit)).toThrow("Invalid artifact");
    await writeFile(join(folder, "dotpop.dmg"), "rebuilt");
    await expect(verifyArtifacts(saved, folder)).rejects.toThrow("Artifact differs");
  } finally { await rm(folder, { recursive: true, force: true }); }
});

test("resuming an older release cannot move latest backwards", () => {
  expect(advancesBuild("10", "9")).toBe(true);
  expect(advancesBuild("9", "10")).toBe(false);
  expect(advancesBuild("10", "10")).toBe(false);
  expect(() => advancesBuild("NaN", "10")).toThrow("Invalid Mac build");
  expect(appcastBuild('<rss><sparkle:version>29</sparkle:version><enclosure sparkle:version="30"/></rss>')).toBe("30");
  expect(() => appcastBuild("<rss/>")).toThrow("no Mac build");
  expect(preservesLatest("1.2.0", "1.10.0")).toBe(false);
  expect(preservesLatest("1.10.0", "1.2.0")).toBe(true);
  expect(preservesLatest("1.2.0", "1.2.0")).toBe(true);
  expect(preservesLatest("1.0.0", "0.0.0-bootstrap.0")).toBe(true);
  expect(preservesLatest("1.0.0", "1.0.0-rc.1")).toBe(true);
  expect(preservesLatest("1.0.0", "2.0.0-rc.1")).toBe(false);
});

test("qualification compares exact historical hashes, including disappeared releases", () => {
  assertQualified({ "1.0.0": "a" }, { "1.0.0": "a" });
  expect(() => assertQualified({}, { "1.0.0": "a" })).toThrow("inventory changed");
  expect(() => assertQualified({ "1.0.0": "a" }, {})).toThrow("inventory changed");
  expect(() => assertQualified({ "1.0.0": "a" }, { "1.0.0": "b" })).toThrow("inventory changed");
});

test("only the successful candidate job on the dispatch commit can publish", () => {
  const record = { documentLineage: DocumentLineage, commit: "c".repeat(40), run: "123", attempt: "1" } as ReleaseRecord;
  const workflow = { id: 123, head_sha: "c".repeat(40), event: "workflow_dispatch", path: ".github/workflows/macos-release.yml" };
  const jobs = [{ name: "Build and test candidate", conclusion: "success" }];
  validateCandidate(record, workflow, jobs);
  expect(() => validateCandidate({ ...record, documentLineage: undefined }, workflow, jobs)).toThrow("unsupported document lineage");
  for (const changed of [{ id: 456 }, { head_sha: "master-after-candidate" }, { event: "push" }, { path: "other.yml" }])
    expect(() => validateCandidate(record, { ...workflow, ...changed }, jobs)).toThrow("identity mismatch");
  for (const conclusion of ["failure", "cancelled", null])
    expect(() => validateCandidate(record, workflow, [{ ...jobs[0]!, conclusion }])).toThrow("did not succeed");
  expect(() => validateCandidate({ ...record, attempt: "2" }, workflow, jobs)).toThrow("new workflow dispatch");
});

test("candidate rebuilds need a fresh dispatch and a protected release environment", () => {
  validateCandidateAttempt("1");
  validateCandidateAttempt(undefined);
  expect(() => validateCandidateAttempt("2")).toThrow("new workflow dispatch");
  validatePublicationSettings({ protection_rules: [{ type: "required_reviewers", reviewers: [{}] }] });
  expect(() => validatePublicationSettings({ protection_rules: [] })).toThrow("manual acceptance");
});

test("publication requires this run's approved review for the release environment", () => {
  validateApproval([{ state: "approved", environments: [{ name: "release" }] }]);
  for (const reviews of [[], [{ state: "rejected", environments: [{ name: "release" }] }], [{ state: "approved", environments: [{ name: "signing" }] }]])
    expect(() => validateApproval(reviews)).toThrow("no approved release review");
});

test("unpublished tags are diagnostic except when they collide with the candidate", () => {
  const tags = ["pop-v9.9.9", "pop-v1.0.0", "macos-v0.1.0"];
  const published = [{ tag_name: "pop-v1.0.0" }];
  expect(checkReleaseTags(tags, published, "pop-v1.1.0")).toEqual(["pop-v9.9.9"]);
  expect(() => checkReleaseTags(tags, published, "pop-v9.9.9")).toThrow("already exists");
  expect(checkReleaseTags(tags, published, "pop-v1.0.0", true)).toEqual(["pop-v9.9.9"]);
});

test("ignoring stray tags never allows drafts or mutable publications into a new candidate", async () => {
  const tags = async () => ["pop-v9.9.9"];
  expect(await checkReleaseHistory("1.1.0", "2", undefined, { list: async () => [], tags })).toBeUndefined();
  const release = { id: 1, tag_name: "pop-v1.0.0", draft: false, prerelease: false, immutable: true, assets: [] };
  for (const changes of [{ draft: true }, { prerelease: true }])
    await expect(checkReleaseHistory("1.1.0", "2", undefined, { list: async () => [{ ...release, ...changes }], tags })).rejects.toThrow("Incomplete publication");
  await expect(checkReleaseHistory("1.1.0", "2", undefined, { list: async () => [{ ...release, immutable: false }], tags })).rejects.toThrow("not immutable");
});

test("publication is complete when npm and GitHub latest agree independently of the website", () => {
  const record = { macVersion: "1.0.0", tag: "pop-v1.0.0", macBuild: "31", commit: "candidate", npmIntegrity: "exact-npm-bytes" } as ReleaseRecord;
  const state = { npm: { version: "1.0.0", dist: { integrity: "exact-npm-bytes" } }, githubTag: "pop-v1.0.0" };
  validatePublication(record, state);
  expect(() => validatePublication(record, { ...state, githubTag: "pop-v0.9.0" })).toThrow("Incomplete publication");
  expect(() => validatePublication(record, { ...state, npm: { ...state.npm, dist: { integrity: "different" } } })).toThrow("Incomplete publication");
});
