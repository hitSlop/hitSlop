import { defineDocument, defineTiny, s, type LiveDocument, type Value } from "../../src/sdk/schema";
import type { Component } from "svelte";
import { attachments } from "../../src/sdk/app/attachments";
import {
  getDocument, isSpeechError, speech, svelteApp, haptics,
  type CaptureMode, type CaptureProps, type SpeechTranscript, type WidgetProps,
} from "../../src/sdk/app/store.svelte";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
type Assert<T extends true> = T;

function declarations(view: Component) {
  const fields = {
    slug: "fixture", title: "Fixture", description: "A typed declaration",
    author: { name: "Test" }, categories: ["utilities"] as const, theme: {},
  };
  const window = { kind: "standard", width: 320, height: 320 } as const;
  defineTiny({ ...fields, window: { ...window, shape: { path: "M0 0L1 0L1 1Z", viewBox: [1, 1], fillRule: "evenodd" } } });
  defineTiny({ ...fields, window: { kind: "skin", width: 320, height: 320, image: "/assets/skin.png" } });
  // @ts-expect-error Skin windows require an imported image URL.
  defineTiny({ ...fields, window: { kind: "skin", width: 320, height: 320 } });
  const mixed = { kind: "skin" as const, width: 320, height: 320, image: "/assets/skin.png", resizable: true };
  // @ts-expect-error A skin cannot carry standard-window flags, even through a variable.
  defineTiny({ ...fields, window: mixed });
  // @ts-expect-error Initial values belong in schema.ts.
  defineTiny({ ...fields, window, initial: {} });
  // @ts-expect-error Components are discovered by filename.
  defineTiny({ ...fields, window, view });
  // @ts-expect-error Commands are discovered by filename.
  defineTiny({ ...fields, window, commands: {} });
  // @ts-expect-error Catalog categories come from the Rust enum.
  defineTiny({ ...fields, categories: ["unknown"], window });
  defineTiny({ ...fields, window, capabilities: ["camera", "microphone", "speech"] });
  // @ts-expect-error Capabilities come from the Rust enum; only implemented ones exist.
  defineTiny({ ...fields, window, capabilities: ["location"] });
  defineDocument({ title: s.string() }, { initial: { title: "Initial" } });
  defineDocument({ title: s.text(), count: s.counter(), note: s.optional(s.string()), done: s.boolean({default:false}) });
  // @ts-expect-error Required fields need initial values.
  defineDocument({ title: s.string() });
  // @ts-expect-error Initial does not influence descriptor inference.
  defineDocument({ title: s.string() }, { initial: { title: 7 } });
  // @ts-expect-error An undefined default does not make a field omittable.
  defineDocument({ done: s.boolean({default:undefined}) });
}
void declarations;

function tactileFeedback() {
  const immediate: void = haptics.selection();
  haptics.impact();
  haptics.impact("soft");
  haptics.notification("warning");
  // @ts-expect-error Feedback styles come from Rust.
  haptics.impact("buzz");
  // @ts-expect-error Notifications express a standard outcome.
  haptics.notification("tap");
  return immediate;
}
void tactileFeedback;

async function collectors(file: File) {
  const schema = defineDocument({ title: s.string(), photo: s.optional(s.string()) }, { initial: {title:""} });
  const doc = getDocument(schema);
  const id: { id: string } = await doc.change(() => ({ id: "kept" }));
  // @ts-expect-error A change() collector cannot be async.
  await doc.change(async tx => { await tx.fields.title.set("Later"); });
  await attachments.import<typeof schema.descriptor>(file, (tx, ref) => tx.fields.photo.set(ref.id));
  // @ts-expect-error An attachment's reference collector cannot be async.
  await attachments.import<typeof schema.descriptor>(file, async (tx, ref) => tx.fields.photo.set(ref.id));
  return id;
}
void collectors;

const board = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
  cover: s.optional(s.object({ id: s.string() })),
});

// App is the only editor role; its components acquire the document their mount edits.
function editorDocument(App: Component) {
  svelteApp(App, { schema: board });
  const doc = getDocument(board);
  type Acquired = Assert<Equal<typeof doc, LiveDocument<typeof board.descriptor>>>;
  type Current = Assert<Equal<typeof doc.current, Value<typeof board.descriptor>>>;
  const done: boolean | undefined = doc.current.tasks[0]?.done;
  // @ts-expect-error getDocument takes the definition, not its descriptor.
  getDocument(board.descriptor);
  return done;
}
void editorDocument;

// Static views are functions of their props: a snapshot, readonly all the way down, and
// attachment URLs; views add their mode and widgets their viewport.
function captureProps(view: CaptureProps<typeof board> & { mode: CaptureMode }, widget: CaptureProps<typeof board> & WidgetProps) {
  type Snapshot = Assert<Equal<typeof view.snapshot, Value<typeof board.descriptor>>>;
  const url: string = view.attachmentURL("cover");
  const modes: CaptureMode[] = ["preview", "export", "icon"];
  // @ts-expect-error A widget receives its viewport, not a mode.
  const mode: CaptureMode = "widget";
  const family: "small" | "medium" | "large" = widget.family;
  // @ts-expect-error The snapshot prop is readonly.
  view.snapshot = widget.snapshot;
  // @ts-expect-error Snapshot fields are readonly.
  view.snapshot.title = "Changed";
  // @ts-expect-error Snapshot lists are readonly.
  view.snapshot.tasks.push(view.snapshot.tasks[0]!);
  // @ts-expect-error Snapshot rows are readonly.
  view.snapshot.tasks[0]!.done = true;
  // @ts-expect-error Nested objects are readonly.
  if (view.snapshot.cover) view.snapshot.cover.id = "other";
  return [url, modes, mode, family];
}
void captureProps;

async function transcription(ref: Awaited<ReturnType<typeof attachments.import>>, error: unknown) {
  const transcript: SpeechTranscript = await speech.transcribe(ref, { locale: "en-US" });
  const text: string = (await speech.transcribe(ref.id)).text;
  // @ts-expect-error A recording is passed by its stored reference or ID, never its bytes.
  await speech.transcribe(new Blob());
  // @ts-expect-error A locale is an identifier string.
  await speech.transcribe(ref, { locale: 7 });
  if (isSpeechError(error)) {
    const code: string = error.code;
    if (error.code === "unavailable" && error.reason === "notDeclared") void code;
  }
  return [transcript.locale, text];
}
void transcription;
