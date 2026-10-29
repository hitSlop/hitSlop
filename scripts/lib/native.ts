// The native tools as scripts run them: requests as the CLI sends them to the document
// engine, and documents created from templates or build stages.
import { PNG } from "pngjs";
import { Database } from "bun:sqlite";
import { appAsset } from "./artifacts";
import { exec } from "../../packages/tinys/src/cli/process";
import { strict as assert } from "node:assert";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execute, findEngine } from "../../packages/tinys/src/cli/engine";
import type {
  EngineMethod,
  EngineRequestFor,
  EngineSuccess,
} from "../../packages/tinys/src/wire/engine";

/** This checkout's document engine. */
export const debugEngine = () => findEngine();
/** Which engine a script runs (the debug build's unless named) and, for an installed one,
 * the folder and environment it runs in. */
export type Placement = { engine?: string; cwd?: string; env?: Record<string, string | undefined> };
/** One request as the CLI sends it; a refusal throws. */
export function engineRequest<M extends EngineMethod>(
  body: EngineRequestFor<M> & { method: M },
  { engine, ...options }: Placement = {},
): Promise<Extract<EngineSuccess, { method: M }>> {
  return execute<M>(body, { binary: engine, timeout: 120_000, ...options });
}
/** An export at `path` that is a real file of its format: past 100 bytes, with the PNG or PDF
 * signature. */
export async function assertExport(path: string, format: "png" | "pdf") {
  const bytes = await readFile(path);
  const signature = format === "png" ? "89504e470d0a1a0a" : "25504446";
  assert.ok(bytes.length > 100, `${path}: an empty ${format}`);
  assert.equal(
    bytes.subarray(0, signature.length / 2).toString("hex"),
    signature,
    `${path}: not a ${format}`,
  );
}
/** A new document at `document` from a built template. */
export async function createDocument(
  template: string,
  document: string,
  { engine, ...placement }: Placement = {},
) {
  await engineRequest(
    { method: "create", from: template, output: document },
    { engine, ...placement },
  );
  return document;
}
/** A new document at `document` from a build stage, made the way a user's are: the CLI's
 * file engine packs the template, and the document engine creates the document from it. */
export async function documentFromStage(stage: string, document: string, options?: Placement) {
  const folder = await mkdtemp(join(tmpdir(), "tinys-fixture-"));
  try {
    const template = join(folder, "template.tiny");
    await engineRequest(
      {
        method: "pack",
        stage,
        file: template,
        app: JSON.parse(await readFile(join(stage, "input.json"), "utf8")),
        renderViews: false,
      },
      options,
    );
    return await createDocument(template, document, options);
  } finally {
    await rm(folder, { recursive: true, force: true });
  }
}

/** Decode pixels so a valid header cannot disguise an empty capture. */
export async function decodedPNG(path: string) {
  const png = PNG.sync.read(await readFile(path));
  assert.ok(png.width > 0 && png.height > 0, `${path}: empty canvas`);
  const colors = new Set<number>();
  for (let i = 0; i < png.data.length; i += 4) {
    if (png.data[i + 3]! > 0) colors.add(png.data.readUInt32BE(i));
    if (colors.size > 1) break;
  }
  assert.ok(colors.size > 1, `${path}: blank capture`);
  return png;
}

/** Count a solid contract patch in a bounded region, avoiding text antialiasing. */
export function assertPixelPatch(
  png: PNG,
  color: string,
  box: { x: number; y: number; width: number; height: number },
  minimum = 256,
) {
  const hex = color.replace("#", "");
  assert.match(hex, /^[0-9a-f]{6}([0-9a-f]{2})?$/i);
  const rgb = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
  let count = 0;
  for (let y = box.y; y < Math.min(png.height, box.y + box.height); y++)
    for (let x = box.x; x < Math.min(png.width, box.x + box.width); x++) {
      const i = (y * png.width + x) * 4;
      if (rgb.every((c, channel) => png.data[i + channel] === c) && png.data[i + 3] === 255)
        count++;
    }
  assert.ok(count >= minimum, `Missing ${color} patch in ${JSON.stringify(box)} (${count} pixels)`);
}

/** PDFKit uses the same text extraction at capture and replay. */
export async function pdfContent(
  path: string,
  cache: string,
): Promise<{ pages: { width: number; height: number; text: string }[] }> {
  const result = await exec(
    [
      "swift",
      "-module-cache-path",
      cache,
      "-e",
      `
import Foundation
import PDFKit
let document = PDFDocument(url: URL(fileURLWithPath: CommandLine.arguments[1]))!
let pages = (0..<document.pageCount).map { index -> [String: Any] in
  let page = document.page(at: index)!, bounds = page.bounds(for: .mediaBox)
  return ["width": bounds.width, "height": bounds.height, "text": page.string ?? ""]
}
print(String(data: try JSONSerialization.data(withJSONObject: ["pages": pages]), encoding: .utf8)!)
`,
      path,
    ],
    { timeout: 120_000 },
  );
  assert.equal(result.code, 0, result.stderr);
  return JSON.parse(result.stdout);
}

/** Render the saved programs, never the stored artwork or rebuilt source. Markup and
 * raster use the current engine's restricted evaluator with the entry's runtime ABI. */
export async function storedCapture(
  document: string,
  kind: "preview" | "icon" | "widget",
  engine: string,
  snapshot: unknown,
  theme: Record<string, string>,
  width: number,
  height: number,
) {
  const db = new Database(document, { readonly: true });
  let runtimeABI: number;
  let attachments: Record<string, string>;
  const blobs = new Map<string, Uint8Array>();
  try {
    runtimeABI = (
      db.query("SELECT runtime_abi FROM app WHERE id=1").get() as { runtime_abi: number }
    ).runtime_abi;
    const rows = db.query("SELECT id, media_type, bytes FROM attachments").all() as {
      id: string;
      media_type: string;
      bytes: Uint8Array;
    }[];
    attachments = Object.fromEntries(rows.map((r) => [r.id, r.media_type]));
    for (const r of rows) blobs.set(r.id, r.bytes);
  } finally {
    db.close();
  }
  const evaluate = async (job: unknown) => {
    const reply = await exec([engine, "--evaluate-command"], {
      stdin: JSON.stringify(job),
      timeout: 120_000,
    });
    assert.equal(reply.code, 0, reply.stderr);
    const output = JSON.parse(reply.stdout);
    assert.notEqual(output.ok, false, output.error);
    return output;
  };
  const widget = kind === "widget";
  const request = widget
    ? {
        snapshot,
        theme,
        attachments,
        viewports: [
          { family: "small", width: 164, height: 164 },
          { family: "medium", width: 344, height: 164 },
          { family: "large", width: 344, height: 344 },
        ],
      }
    : {
        kind,
        format: "png",
        snapshot,
        theme,
        attachments,
        width: kind === "icon" ? 512 : width,
        height: kind === "icon" ? 512 : height,
      };
  const batch = await evaluate({
    runtimeABI,
    mode: widget ? "widgetMarkup" : "markup",
    bundle: appAsset(document, `${kind}.js`),
    request: JSON.stringify(request),
  });
  assert.equal(batch.frames.length, widget ? 3 : 1, `${kind}: incomplete frames`);
  const images: Record<string, string> = {};
  for (const frame of batch.frames) {
    assert.ok(frame.html.length > 0, `${kind}: empty markup`);
    for (const match of (frame.html + frame.css).matchAll(/tiny-attachment:([a-f0-9]{64})/g)) {
      const id = match[1]!;
      const bytes = blobs.get(id);
      assert.ok(bytes, `Missing capture attachment ${id}`);
      images[id] = `data:${attachments[id]};base64,${Buffer.from(bytes).toString("base64")}`;
    }
  }
  const raster = await evaluate({
    runtimeABI,
    mode: widget ? "widget" : "capture",
    bundle: "",
    request: "",
    raster: { batch, images },
  });
  assert.equal(raster.images.length, batch.frames.length, `${kind}: incomplete images`);
  return {
    frames: batch.frames as { html: string; css: string; width: number; height: number }[],
    images: raster.images as string[],
  };
}
