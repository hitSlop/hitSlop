import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { exec } from "../../scripts/lib/test-process";

const payload = Buffer.from([0, 255, 10, 13, 128, 65]).toString("base64");

async function download(errors: string[]) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-release-download-"));
  try {
    const attempts = join(folder, "attempts");
    await writeFile(attempts, "0");
    await writeFile(join(folder, "gh"), `#!${process.execPath}
import { readFileSync, writeFileSync } from "node:fs";
const attempt = Number(readFileSync(process.env.RELEASE_DOWNLOAD_ATTEMPTS, "utf8")) + 1;
writeFileSync(process.env.RELEASE_DOWNLOAD_ATTEMPTS, String(attempt));
if (process.argv.at(-1) !== "repos/hitSlop/hitSlop/releases/assets/123") process.exit(99);
const error = ${JSON.stringify(errors)}[attempt - 1];
if (error) {
  process.stdout.write("partial response that must be discarded");
  process.stderr.write(error);
  process.exit(1);
}
process.stdout.write(Buffer.from(${JSON.stringify(payload)}, "base64"));
`, { mode: 0o755 });
    const result = await exec([process.execPath, "-e", `
import { assetBytes } from ${JSON.stringify(resolve("scripts/lib/release-assets.ts"))};
const bytes = await assetBytes(123);
process.stdout.write(bytes.toString("base64"));
`], {
      env: { ...process.env, PATH: `${folder}:${process.env.PATH}`, RELEASE_DOWNLOAD_ATTEMPTS: attempts },
      timeout: 15_000,
    });
    return { ...result, attempts: Number(await readFile(attempts, "utf8")) };
  } finally { await rm(folder, { recursive: true, force: true }); }
}

test("a successful release asset download preserves binary bytes without retry", async () => {
  const result = await download([]);
  expect(result.code).toBe(0);
  expect(result.stdout).toBe(payload);
  expect(result.attempts).toBe(1);
});

test.each([
  "gh: server unavailable (HTTP 502)",
  "unexpected end of JSON input",
  "unexpected EOF",
  "Get https://api.github.com/: dial tcp: i/o timeout",
])("a transient release asset failure recovers: %s", async error => {
  const result = await download([error]);
  expect(result.code).toBe(0);
  expect(result.stdout).toBe(payload);
  expect(result.attempts).toBe(2);
});

test.each([401, 403, 404])("HTTP %s fails without retry", async status => {
  const result = await download([`gh: request refused (HTTP ${status})`]);
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain(`HTTP ${status}`);
  expect(result.attempts).toBe(1);
});

test("persistent transient failures stop after three retries", async () => {
  const result = await download(Array(4).fill("gh: server unavailable (HTTP 503)"));
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain("Cannot download release asset 123");
  expect(result.stderr).toContain("HTTP 503");
  expect(result.attempts).toBe(4);
}, 15_000);

test("a permanent failure after a transient one stops further retries", async () => {
  const result = await download(["unexpected end of JSON input", "gh: not found (HTTP 404)"]);
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain("HTTP 404");
  expect(result.attempts).toBe(2);
});
