import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { exec } from "../../scripts/lib/test-process";

// Exercise the shell entry point with ticket-service failures at its process boundary.
// The sleep stand-in records backoff without making this test wait for an outage.
async function stapler(action: "staple" | "validate", responses: number[]) {
  const folder = await mkdtemp(join(tmpdir(), "tinys-stapler-"));
  try {
    await writeFile(join(folder, "xcrun"), `#!/bin/sh
set -eu
[ "$1" = stapler ] && [ "$2" = "$STAPLER_ACTION" ] && [ "$3" = "$STAPLER_TARGET" ] || exit 64
attempt=$(cat "$STAPLER_STATE")
attempt=$((attempt + 1))
echo "$attempt" > "$STAPLER_STATE"
case "$attempt" in
${responses.map((code, index) => `  ${index + 1}) exit ${code} ;;`).join("\n")}
  *) exit 99 ;;
esac
`, { mode: 0o755 });
    await writeFile(join(folder, "sleep"), '#!/bin/sh\necho "$1" >> "$STAPLER_DELAYS"\n', { mode: 0o755 });
    await writeFile(join(folder, "attempts"), "0\n");
    await writeFile(join(folder, "delays"), "");
    const target = join(folder, "App with spaces.app");
    const result = await exec(["/bin/sh", resolve("scripts/release/stapler.sh"), action, target], {
      env: {
        PATH: `${folder}:/usr/bin:/bin`,
        STAPLER_ACTION: action, STAPLER_TARGET: target,
        STAPLER_STATE: join(folder, "attempts"), STAPLER_DELAYS: join(folder, "delays"),
      },
      timeout: 5_000,
    });
    return {
      ...result,
      attempts: Number(await readFile(join(folder, "attempts"), "utf8")),
      delays: (await readFile(join(folder, "delays"), "utf8")).trim().split("\n").filter(Boolean).map(Number),
    };
  } finally { await rm(folder, { recursive: true, force: true }); }
}

test.each(["staple", "validate"] as const)("%s recovers after ticket-service failures", async action => {
  const result = await stapler(action, [68, 68, 0]);
  expect(result.code).toBe(0);
  expect(result.attempts).toBe(3);
  expect(result.delays).toEqual([10, 20]);
});

test("a persistent ticket-service failure stops after bounded retries", async () => {
  const result = await stapler("staple", [68, 68, 68, 68]);
  expect(result.code).toBe(68);
  expect(result.attempts).toBe(4);
  expect(result.delays).toEqual([10, 20, 40]);
});

test.each([0, 64, 65, 66, 73, 77])("stapler result %s is preserved without retry", async code => {
  const result = await stapler("validate", [code]);
  expect(result.code).toBe(code);
  expect(result.attempts).toBe(1);
  expect(result.delays).toEqual([]);
});

test("an invalid ticket after a service outage still fails", async () => {
  const result = await stapler("staple", [68, 65]);
  expect(result.code).toBe(65);
  expect(result.attempts).toBe(2);
  expect(result.delays).toEqual([10]);
});
