// Replays the prepared compatibility corpus through this build's CLI and engine: every
// saved document of every release reads as that release recorded, renders with its own old
// app, keeps its attachments, takes a CLI edit and reopens to the recorded result; every
// template master still creates documents. Old files are checked, not old programs. The Rust
// and Swift corpus tests cover the rest (docs/testing.md).
import { EngineProtocol } from "../../packages/tinys/src/schema/constants";
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { strict as assert } from "node:assert";
import { mkdtemp, readdir, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  assertExport,
  createDocument,
  decodedPNG,
  assertPixelPatch,
  pdfContent,
  storedCapture,
} from "../../scripts/lib/native";
import {
  documents,
  copyCorpusDocument,
  documentEngine,
  readJSON,
  releases,
  savedState,
  tiny,
  tinyJSON,
  type Expected,
  type Scenario,
  type CommandProbe,
  type ExportGolden,
  type ThemeScenario,
} from "../../scripts/compat/corpus";
import { sha256, useTestRegistry } from "../../scripts/lib/artifacts";
import { evaluateStored } from "../../scripts/compat/commands";
import { exec } from "../../packages/tinys/src/cli/process";
import { request } from "../../packages/tinys/src/cli/engine";
useTestRegistry();

const entries = await Promise.all((await releases()).map(async entry => ({
  ...entry,
  documents: await documents(entry.root),
  templates: await readdir(join(entry.root, "templates")),
})));
// Resolved, as the CLI prints the paths it writes (/var is a link on macOS).
let scratch: string;
beforeAll(async () => {
  scratch = await realpath(await mkdtemp(join(tmpdir(), "tinys-compat-")));
});
afterAll(() => rm(scratch, { recursive: true, force: true }));

test("the corpus has entries to replay", () => {
  if (process.env.TINYS_COMPAT_BOOTSTRAP !== "1") expect(entries.length).toBeGreaterThan(0);
});
for (const { name, root, release, documents: saved, templates } of entries)
  describe(`corpus/${name} (${release.frozen ? "frozen" : "replaceable"})`, () => {
    for (const document of saved)
      test(`${document} reads as recorded, renders its saved scenario, keeps its attachments and reopens edited`, async () => {
        const copy = join(scratch, `${name}-${document}.tiny`);
        const fresh = async () => {
          await rm(copy, { force: true });
          await copyCorpusDocument(join(root, "documents", document + ".tiny"), copy);
        };
        await fresh();
        const expected = await readJSON<Expected>(join(root, "expected", document + ".json"));
        assert.ok(expected, `${document}: missing expected state`);
        // Saved formats survive upgrades; the CLI transport is the current protocol.
        const negotiated = await exec(
          [documentEngine(), "--client-protocol", String(EngineProtocol.version)],
          {
            stdin: JSON.stringify({ method: "get", documentPath: copy }),
            timeout: 120_000,
          },
        );
        assert.equal(negotiated.code, 0, negotiated.stderr);
        assert.equal(JSON.parse(negotiated.stdout).ok, true);
        expect(JSON.parse(negotiated.stdout).state.value).toEqual(expected.value);
        if (release.markers.protocol !== EngineProtocol.version) {
          const before = await Bun.file(copy).bytes();
          const refused = await exec([documentEngine(), "--client-protocol", String(release.markers.protocol)], {
            stdin: JSON.stringify({ method: "get", documentPath: copy }), timeout: 120_000,
          });
          assert.equal(refused.code, 2);
          expect(await Bun.file(copy).bytes()).toEqual(before);
        }
        expect(await savedState(copy)).toEqual(expected);
        for (const { id, byteLength } of expected.attachments) {
          const output = join(scratch, id);
          const exported = await tiny(["attachments", "export", copy, id, "--output", output]);
          assert.equal(exported.code, 0, `attachment ${id}: ${exported.stderr.trim()}`);
          const bytes = await Bun.file(output).bytes();
          expect(bytes.length).toBe(byteLength);
          expect(sha256(bytes)).toBe(id);
          await rm(output);
        }
        const golden = await readJSON<ExportGolden>(join(root, "exports", document + ".json"));
        assert.ok(golden, `${document}: missing export goldens`);
        const info = await tinyJSON(["inspect", copy]);
        const window = info.window;
        assert.ok(window?.width > 0 && window?.height > 0, "Missing saved window");
        for (const format of ["png", "pdf"] as const) {
          const output = join(scratch, `${name}-${document}.${format}`);
          const { code, stderr } = await tiny([
            "export",
            copy,
            "--format",
            format,
            "--output",
            output,
          ]);
          assert.equal(code, 0, `the old app did not render: ${stderr.trim()}`);
          await assertExport(output, format);
          if (format === "png") {
            const png = await decodedPNG(output);
            expect([png.width, png.height]).toEqual([golden.png.width, golden.png.height]);
            expect(png.width).toBe(window.width * 2);
            // Match solid patch positions from the stored image without pinning glyph
            // antialiasing or the renderer's entire pixel digest.
            const recorded = await decodedPNG(join(root, "exports", document + ".png"));
            expect(sha256(recorded.data)).toBe(golden.png.rgbaSha256);
            const patchColors: string[] = ["#335577", expected.theme.effective.accent!];
            for (const color of patchColors) {
              const rgb = [1, 3, 5].map((i) => parseInt(color.slice(i, i + 2), 16));
              let patch: { x: number; y: number; width: number; height: number } | undefined;
              for (let y = 0; y < recorded.height - 16 && !patch; y++)
                for (let x = 0; x < recorded.width - 16 && !patch; x++) {
                  const solid = [0, 15].every((dy) =>
                    [0, 15].every((dx) => {
                      const i = ((y + dy) * recorded.width + x + dx) * 4;
                      return (
                        rgb.every((c, ch) => recorded.data[i + ch] === c) &&
                        recorded.data[i + 3] === 255
                      );
                    }),
                  );
                  if (solid) patch = { x, y, width: 16, height: 16 };
                }
              assert.ok(patch, `Golden lost ${color} patch`);
              assertPixelPatch(png, color, patch);
            }
          } else {
            const pdf = await pdfContent(output, join(scratch, "swift-cache"));
            expect(pdf.pages.map(({ width, height }) => ({ width, height }))).toEqual(
              golden.pdf.pages.map(({ width, height }) => ({ width, height })),
            );
            expect(pdf.pages.map((p) => p.text)).toEqual(golden.pdf.pages.map((p) => p.text));
            const text = pdf.pages.map((p) => p.text).join("\n");
            expect(text).toContain("中文");
            expect(text).toContain("😀");
            expect(text).not.toContain("Hidden in capture");
          }
          await rm(output);
        }
        for (const kind of ["preview", "icon", "widget"] as const) {
          const capture = await storedCapture(
            copy,
            kind,
            documentEngine(),
            expected.value,
            expected.theme.effective,
            window.width,
            window.height,
          );
          for (let i = 0; i < capture.images.length; i++) {
            const frame = capture.frames[i]!;
            expect(frame.html).toContain((expected.value as { title: string }).title);
            if (kind !== "icon") expect(frame.html).toContain("中文 😀");
            const output = join(scratch, `${document}-${kind}-${i}.png`);
            await Bun.write(output, Buffer.from(capture.images[i]!, "base64"));
            const png = await decodedPNG(output);
            // Icons render at 1x; previews, exports and widgets at 2x.
            const scale = kind === "icon" ? 1 : 2;
            expect(png.width).toBe(frame.width * scale);
            if (kind !== "preview") expect(png.height).toBe(frame.height * scale);
            assertPixelPatch(png, expected.theme.effective.accent!, {
              x: 0,
              y: 0,
              width: png.width,
              height: png.height,
            });
            if (kind === "preview")
              assertPixelPatch(png, "#335577", {
                x: 0,
                y: 0,
                width: png.width,
                height: png.height,
              });
            if (kind === "icon")
              expect(frame.html).toContain(String((expected.value as { hits: number }).hits));
            if (kind === "widget") expect(frame.html).toContain("rows /");
            await rm(output);
          }
        }
        const scenario = await readJSON<Scenario>(join(root, "scenarios", document + ".json"));
        assert.ok(scenario, `${document}: missing CLI scenario`);
        await fresh();
        await tinyJSON(["batch", copy, "--ops", JSON.stringify(scenario.ops)]);
        expect((await tinyJSON(["get", copy, "--snapshot"])).value).toEqual(scenario.value);
        expect((await savedState(copy)).attachments).toEqual(expected.attachments);

        const themes = await readJSON<ThemeScenario>(join(root, "themes", document + ".json"));
        assert.ok(themes?.steps.length, `${document}: missing theme scenarios`);
        await fresh();
        for (const step of themes.steps) {
          const args = step.values
            ? ["theme", "set", copy, "--values", JSON.stringify(step.values)]
            : ["theme", "reset", copy, "--token", step.reset!];
          await tinyJSON(args);
          const theme = await tinyJSON(["theme", "get", copy]);
          expect({ overrides: theme.overrides, effective: theme.effective }).toEqual(step.state);
        }

        const probes = await readJSON<CommandProbe[]>(join(root, "commands", document + ".json"));
        // Capture records commands for CLI-saved documents; page-saved copies are
        // replayed by their page and CLI scenarios instead.
        assert.ok(probes || document.endsWith(".page"), `${document}: missing command probes`);
        if (!probes)
          console.info(
            `SKIP ${document}: commands are recorded for CLI-saved documents; page edits replay separately`,
          );
        if (probes) {
          assert.ok(
            Array.isArray(probes) && probes.length > 0,
            "Expected command probe array",
          );
          for (const probe of probes) {
            assert.ok(
              probe.channel === "call" || probe.channel === "evaluate",
              "Missing command probe channel",
            );
            await fresh();
            const state = await tinyJSON(["get", copy, "--snapshot"]);
            if (probe.channel === "evaluate") {
              const evaluated = await evaluateStored(copy, state, probe);
              expect(evaluated).toEqual(probe.evaluated);
              if (evaluated.ok && evaluated.intents.length)
                await tinyJSON(["batch", copy, "--ops", JSON.stringify(evaluated.intents)]);
              expect((await tinyJSON(["get", copy, "--snapshot"])).value).toEqual(probe.value);
              if (!evaluated.ok)
                expect((await tinyJSON(["get", copy, "--snapshot"])).value).toEqual(state.value);
            }
            // The actual owner prepares defaults, rejects bad args and admits intents.
            await fresh();
            const called = await request(
              { method: "call", documentPath: copy, command: probe.name, args: probe.args },
              { binary: documentEngine(), timeout: 120_000 },
            );
            expect(called.ok).toBe(probe.expected.ok);
            if (!called.ok) {
              if (probe.expected.code) assert.equal(called.code, probe.expected.code);
              if (probe.expected.refused) expect(called.reason).toBe("refused");
              if (probe.expected.errorIncludes)
                expect(called.error).toContain(probe.expected.errorIncludes);
              expect((await tinyJSON(["get", copy, "--snapshot"])).value).toEqual(state.value);
            } else {
              const value = (await tinyJSON(["get", copy, "--snapshot"])).value;
              if (probe.name === "bump") {
                expect(value.hits).toBe(state.value.hits + 2);
                expect(value.lastHit).toBe(state.value.hits + 2);
                expect(called.result).toBe(state.value.hits + 2);
              } else if (probe.name === "shapes") {
                // The owner supplies a fresh time/seed and mints row IDs. Compare
                // their relationships and edits, rather than pretending they are fixed.
                expect(value.hits).toBe(state.value.hits + 1);
                expect(value.settings.hits).toBe(state.value.settings.hits + 1);
                expect(value.title).toBe("Command title");
                expect(value.rows.some((r: { text: string }) => r.text === "Command anchor")).toBe(
                  true,
                );
                expect(value.rows.some((r: { text: string }) => r.text === "Before")).toBe(false);
                expect(value.colors).toEqual(["black", "blue"]);
                expect(value.note).toBeUndefined();
                expect(value.place).toBeUndefined();
              } else {
                expect(value).toEqual(probe.value);
                expect(called.result).toEqual((probe.evaluated as { result: unknown }).result);
              }
            }
          }
        }
      }, 120_000);

    for (const file of templates)
      test(`template ${file} creates the document its release did`, async () => {
        const output = join(scratch, `${name}-created-${file}`);
        await rm(output, { force: true });
        await createDocument(join(root, "templates", file), output, { engine: documentEngine() });
        const initial = await readJSON(
          join(root, "expected", `new-${file.slice(0, -".tiny".length)}.json`),
        );
        assert.notEqual(initial, undefined, `${file}: missing initial state`);
        expect((await tinyJSON(["get", output, "--snapshot"])).value).toEqual(initial);
      }, 120_000);
  });
