import { afterEach, expect, test } from "bun:test";
import { ownerSpeech } from "../../src/shell/speech";
import { isSpeechError } from "../../src/sdk/errors";

// A scripted native bridge: it records each request and answers with `reply`.
const root = globalThis as any;
const before = root.webkit;
afterEach(() => {
  root.webkit = before;
});
function host(reply: (request: any) => unknown) {
  const requests: any[] = [];
  root.webkit = {
    messageHandlers: {
      hitslop: {
        postMessage: async (body: string) => {
          const request = JSON.parse(body);
          requests.push(request);
          return JSON.stringify(reply(request));
        },
      },
    },
  };
  return requests;
}
const id = "0123456789abcdef".repeat(4);
const answer = (outcome: object) => ({ ok: true, method: "speech.transcribe", outcome });
const failure = (promise: Promise<unknown>) => promise.then(() => undefined, (error) => error);

test("a stored recording is transcribed by its attachment ID, never its bytes", async () => {
  const requests = host(() => answer({ status: "transcribed", text: "Buy oat milk.", locale: "en-US" }));
  const ref = { id, byteLength: 4096, mimeType: "audio/mp4", name: "note.m4a" };
  expect(await ownerSpeech().transcribe(ref, { locale: "en-US" })).toEqual({ text: "Buy oat milk.", locale: "en-US" });
  expect(await ownerSpeech().transcribe(id)).toEqual({ text: "Buy oat milk.", locale: "en-US" });
  expect(requests).toEqual([
    { method: "speech.transcribe", attachment: id, locale: "en-US" },
    { method: "speech.transcribe", attachment: id },
  ]);
});

test("unavailable speech says why, including an app that did not declare it", async () => {
  for (const reason of ["notDeclared", "locale", "permission", "host", "system", "format", "futureReason"]) {
    host(() => answer({ status: "unavailable", reason, message: `Unavailable: ${reason}` }));
    const error = await failure(ownerSpeech().transcribe(id));
    expect(isSpeechError(error)).toBe(true);
    expect([error.code, error.reason, error.message]).toEqual(["unavailable", reason, `Unavailable: ${reason}`]);
  }
});

test("a cancelled or failed transcription rejects with its code", async () => {
  host(() => answer({ status: "cancelled", message: "The page was replaced" }));
  let error = await failure(ownerSpeech().transcribe(id));
  expect([isSpeechError(error), error.code, error.reason]).toEqual([true, "cancelled", undefined]);
  // The document closing before the host answered also ends the request.
  host(() => ({ ok: false, code: "closing", error: "Document is closing" }));
  error = await failure(ownerSpeech().transcribe(id));
  expect([error.code, error.message]).toEqual(["cancelled", "Document is closing"]);
  host(() => answer({ status: "failed", message: "The recording could not be read" }));
  error = await failure(ownerSpeech().transcribe(id));
  expect([error.code, error.message]).toEqual(["failed", "The recording could not be read"]);
  host(() => ({ ok: false, code: "rejected", reason: "invalid_request", error: "Invalid page request" }));
  expect((await failure(ownerSpeech().transcribe(id))).code).toBe("failed");
});

test("a malformed clip or locale is refused before the host is asked", async () => {
  const requests = host(() => answer({ status: "transcribed", text: "", locale: "en" }));
  for (const clip of ["clip.m4a", { id: "../ui.js" }, null, 7] as any[])
    expect(await failure(ownerSpeech().transcribe(clip))).toBeInstanceOf(TypeError);
  for (const locale of ["", "en US", "-en", "a".repeat(36), 7] as any[]) {
    const error = await failure(ownerSpeech().transcribe(id, { locale }));
    expect([isSpeechError(error), error.code, error.reason]).toEqual([true, "unavailable", "locale"]);
  }
  expect(requests).toEqual([]);
});
