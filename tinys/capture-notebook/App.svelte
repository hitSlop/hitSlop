<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { AlertDialog, Dialog } from "bits-ui";
  import { attachments, draft, EditableText, getDocument, notify } from "@hitslop/tinys/svelte";
  import Mic from "@lucide/svelte/icons/mic";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import Camera from "./Camera.svelte";
  import Recorder from "./Recorder.svelte";
  import Print from "./Print.svelte";
  import Player from "./Player.svelte";
  import {
    addNote, fileName, formatBytes, formatLabel, keepPhoto, keepRecording, notebookView, removeEntry, roomFor, stamp, summary,
    type Entry,
  } from "./notebook";

  const doc = getDocument(schema);
  const view = $derived(notebookView(doc.current));
  // Which photo is open large and which entry is being removed are view state.
  let viewingId = $state<string | null>(null);
  let removingId = $state<string | null>(null);
  const viewing = $derived(view.entries.find(entry => entry.$id === viewingId));
  const removing = $derived(view.entries.find(entry => entry.$id === removingId));
  let composer = $state<HTMLInputElement>();
  const newNote = draft(text => doc.change(tx => addNote(tx, text, new Date().toISOString())));

  async function keepShot({ blob, width, height }: { blob: Blob; width: number; height: number }) {
    const full = roomFor(doc.current, blob.size);
    if (full) throw new Error(full);
    const now = new Date();
    const file = new File([blob], fileName("photo", now, "jpg"), { type: "image/jpeg" });
    await attachments.import<typeof schema.descriptor>(file, (tx, ref) => { keepPhoto(tx, ref, now.toISOString(), width, height); });
    notify(`Photo kept · ${formatBytes(blob.size)}`);
  }
  async function keepClip({ blob, ext, seconds }: { blob: Blob; ext: string; seconds: number }) {
    const full = roomFor(doc.current, blob.size);
    if (full) throw new Error(full);
    const now = new Date(Date.now() - seconds * 1000);
    const file = new File([blob], fileName("recording", now, ext), { type: blob.type });
    await attachments.import<typeof schema.descriptor>(file, (tx, ref) => { keepRecording(tx, ref, now.toISOString(), seconds); });
    notify(`Recording kept · ${formatBytes(blob.size)}`);
  }
  async function addTyped(event: SubmitEvent) {
    if (await newNote.submit(event)) composer?.focus();
  }
  async function confirmRemove() {
    const entry = removing;
    removingId = null;
    if (!entry) return;
    await doc.change(tx => removeEntry(tx, doc.current, entry.$id));
    notify(`${noun(entry)} removed.`);
  }
  function noun(entry: Entry) {
    return entry.kind === "photo" ? "Photo" : entry.kind === "audio" ? "Recording" : "Note";
  }
  function describe(entry: Entry) {
    return entry.caption.trim() || `${noun(entry).toLowerCase()} from ${stamp(entry.at)}`;
  }
</script>

<main class="cn-shell">
  <section class="cn-body" aria-label="Capture">
    <Camera onphoto={keepShot} />
    <Recorder onrecording={keepClip} />
  </section>

  <section class="cn-page" aria-label="Notebook">
    <header class="cn-page-head">
      <EditableText class="cn-title" field={doc.fields.title} label="Notebook name" placeholder="Name this notebook" onenter={() => composer?.focus()} />
      <p class="cn-summary">{summary(view)}{view.totalBytes ? ` · ${formatBytes(view.totalBytes)} kept` : ""}</p>
    </header>
    <form class="cn-composer" onsubmit={addTyped}>
      <PenLine size={17} aria-hidden="true" />
      <input bind:this={composer} bind:value={newNote.value} aria-label="New note" placeholder="Jot a note…" />
      <button type="submit" aria-label="Add note" disabled={!newNote.ready}><Plus size={18} /></button>
    </form>
    <ol class="cn-entries">
      {#each view.entries as entry (entry.$id)}
        {@const row = doc.at(entry)}
        <li class="cn-entry" data-kind={entry.kind}>
          {#if entry.kind === "photo" && entry.file}
            <button class="cn-print-button" onclick={() => (viewingId = entry.$id)} aria-label={`Open ${describe(entry)}`}>
              <Print src={attachments.url(entry.file.id)} alt="" width={entry.width} height={entry.height} />
            </button>
          {:else if entry.kind === "audio"}
            <span class="cn-mark cn-mark-audio" aria-hidden="true"><Mic size={20} /></span>
          {:else}
            <span class="cn-mark cn-mark-note" aria-hidden="true"><PenLine size={18} /></span>
          {/if}
          <div class="cn-entry-body">
            <p class="cn-stamp">
              <span>{stamp(entry.at)}</span>
              {#if entry.file}<span>{formatLabel(entry)} · {formatBytes(entry.file.byteLength)}</span>{/if}
            </p>
            <EditableText
              class="cn-caption"
              field={row.caption}
              label={entry.kind === "note" ? "Note" : `Caption for ${noun(entry).toLowerCase()} from ${stamp(entry.at)}`}
              placeholder={entry.kind === "note" ? "Empty note" : entry.kind === "photo" ? "Add a caption" : "What was said?"}
            />
            {#if entry.kind === "audio" && entry.file}
              <Player src={attachments.url(entry.file.id)} seconds={entry.seconds ?? 0} label={describe(entry)} />
            {/if}
          </div>
          <button class="cn-remove" aria-label={`Remove ${describe(entry)}`} onclick={() => (removingId = entry.$id)}><Trash size={16} /></button>
        </li>
      {:else}
        <li class="cn-empty">
          <strong>An empty page.</strong>
          <span>Take a photo, record a sound or jot a note. Everything you keep stays in this document.</span>
        </li>
      {/each}
    </ol>
  </section>
</main>

<Dialog.Root open={!!viewing} onOpenChange={open => { if (!open) viewingId = null; }}>
  <Dialog.Portal>
    <Dialog.Overlay class="cn-overlay" />
    <Dialog.Content class="cn-lightbox">
      {#if viewing?.file}
        <Dialog.Title class="cn-lightbox-title">{viewing.caption.trim() || "Photo"}</Dialog.Title>
        <Print size="large" src={attachments.url(viewing.file.id)} alt={describe(viewing)} width={viewing.width} height={viewing.height} />
        <Dialog.Description class="cn-lightbox-meta">
          {stamp(viewing.at)} · {viewing.width}×{viewing.height} · {formatBytes(viewing.file.byteLength)}
        </Dialog.Description>
      {/if}
      <Dialog.Close class="cn-icon-button cn-lightbox-close" aria-label="Close photo"><X size={20} /></Dialog.Close>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<AlertDialog.Root open={!!removing} onOpenChange={open => { if (!open) removingId = null; }}>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="cn-overlay" />
    <AlertDialog.Content class="cn-dialog">
      {#if removing}
        <AlertDialog.Title class="cn-dialog-title">Remove this {noun(removing).toLowerCase()}?</AlertDialog.Title>
        <AlertDialog.Description class="cn-dialog-text">
          {removing.file
            ? `“${describe(removing)}” leaves the notebook, and its file is deleted from the document when it closes.`
            : `“${describe(removing)}” leaves the notebook.`}
        </AlertDialog.Description>
      {/if}
      <div class="cn-dialog-actions">
        <AlertDialog.Cancel class="cn-button cn-button-quiet">Keep it</AlertDialog.Cancel>
        <AlertDialog.Action class="cn-button cn-button-danger" onclick={confirmRemove}>Remove</AlertDialog.Action>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>
