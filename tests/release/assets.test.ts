import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { exec } from "../../scripts/lib/test-process";

const payload = Buffer.concat([
  Buffer.from([0, 255, 10, 13, 128, 65]),
  Buffer.from("\r\n\r\nHTTP/1.1 404 Not Found\n\nbody"),
]).toString("base64");
type DownloadResponse = { status?: number; error?: string; body?: string };

async function download(responses: DownloadResponse[], lineEnding = "\r\n") {
  const folder = await mkdtemp(join(tmpdir(), "tinys-release-download-"));
  try {
    const attempts = join(folder, "attempts");
    await writeFile(attempts, "0");
    await writeFile(join(folder, "gh"), `#!${process.execPath}
import { readFileSync, writeFileSync } from "node:fs";
const attempt = Number(readFileSync(process.env.RELEASE_DOWNLOAD_ATTEMPTS, "utf8")) + 1;
writeFileSync(process.env.RELEASE_DOWNLOAD_ATTEMPTS, String(attempt));
if (process.argv.at(-1) !== "repos/hitSlop/tinys/releases/assets/123") process.exit(99);
const response = ${JSON.stringify(responses)}[attempt - 1] ?? { status: 200 };
if (process.argv.includes("--include") && response.status !== undefined) {
  process.stdout.write("HTTP/2.0 " + response.status + " Status\\nContent-Type: application/octet-stream" + ${JSON.stringify(lineEnding + lineEnding)});
}
if (response.error) {
  process.stdout.write(response.body ?? "partial response that must be discarded");
  process.stderr.write(response.error);
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

test.each(["\r\n", "\n"])("a successful release asset download preserves binary bytes with %j headers", async lineEnding => {
  const result = await download([], lineEnding);
  expect(result.code).toBe(0);
  expect(result.stdout).toBe(payload);
  expect(result.attempts).toBe(1);
});

test.each([
  { status: 502, error: "gh: server unavailable (HTTP 502)" },
  { error: "unexpected end of JSON input" },
  { error: "unexpected EOF" },
  { error: "Get https://api.github.com/: dial tcp: i/o timeout" },
  { status: 503, error: "gh: Service unavailable", body: '{"errors":"Service unavailable"}' },
  { error: 'Get "https://api.github.com/": EOF' },
  { status: 200, error: 'Get "https://release-assets.githubusercontent.com/": EOF' },
])("a transient release asset failure recovers: %j", async response => {
  const result = await download([response]);
  expect(result.code).toBe(0);
  expect(result.stdout).toBe(payload);
  expect(result.attempts).toBe(2);
});

test.each([401, 403, 404])("HTTP %s fails without retry", async status => {
  const result = await download([{ status, error: `gh: request refused (HTTP ${status})` }]);
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain(`HTTP ${status}`);
  expect(result.attempts).toBe(1);
});

test.each(["unexpected end of JSON input", 'Get "https://api.github.com/": EOF'])("HTTP 404 takes precedence over ambiguous stderr: %s", async error => {
  const result = await download([{ status: 404, error, body: "" }]);
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain("HTTP 404");
  expect(result.attempts).toBe(1);
});

test("a successful process without response headers is refused", async () => {
  const result = await download([{}]);
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain("HTTP response");
  expect(result.attempts).toBe(1);
});

test("persistent transient failures stop after three retries", async () => {
  const result = await download(Array(4).fill({ status: 503, error: "gh: Service unavailable" }));
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain("Cannot download release asset 123");
  expect(result.stderr).toContain("HTTP 503");
  expect(result.attempts).toBe(4);
}, 15_000);

test("a permanent failure after a transient one stops further retries", async () => {
  const result = await download([{ error: "unexpected end of JSON input" }, { status: 404, error: "gh: not found (HTTP 404)" }]);
  expect(result.code).not.toBe(0);
  expect(result.stderr).toContain("HTTP 404");
  expect(result.attempts).toBe(2);
});
