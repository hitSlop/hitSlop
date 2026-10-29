<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { onDestroy, onMount } from "svelte";
  import { AlertDialog, Select } from "bits-ui";
  import { isDocumentError } from "@hitslop/tinys";
  import { attachments, EditableText, getDocument, isSpeechError, notify, speech, type SpeechError } from "@hitslop/tinys/svelte";
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import Trash from "@lucide/svelte/icons/trash";
  import AudioLines from "@lucide/svelte/icons/audio-lines";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import {
    addRecording, addTypedNote, baseType, durationLabel, extensionFor, languages,
    MAX_ATTACHMENT_BYTES, MAX_RECORDING_MS, padView, removeNote, saveTranscription, setLocale, sizeLabel, type Clip, type Note,
  } from "./notes";
  import { describeProblem, Recording, recordingType, type CaptureProblem } from "./recorder";
  import NoteMeta from "./NoteMeta.svelte";
  import Spiral from "./Spiral.svelte";

  const doc = getDocument(schema);
  const view = $derived(padView(doc.current));

  // Everything below is view state. The live microphone, the level, the clock and an
  // unfinished recording are never saved; only a kept clip becomes an attachment.
  type Phase = "idle" | "starting" | "recording" | "saving";
  let phase = $state<Phase>("idle");
  let recording: Recording | null = null;
  let elapsed = $state(0);
  let level = $state(0);
  let problem = $state<CaptureProblem | null>(null);
  let frame = 0;
  let destroyed = false;
  const willRecordAs = typeof window === "undefined" ? undefined : recordingType();

  type Status = { kind: "working" } | { kind: "problem"; tone: "unavailable" | "cancelled" | "failed"; text: string };
  let statuses = $state<Record<string, Status>>({});
  /** A reason that applies to every note (this host, the device, the permission). */
  let speechNotice = $state<string | null>(null);

  let player = $state<HTMLAudioElement>();
  let playingId = $state<string | null>(null);
  let paused = $state(true);
  let position = $state(0);

  let confirmReplace = $state<string | null>(null);
  let confirmDelete = $state<string | null>(null);

  const language = $derived(doc.current.locale);
  const languageOptions = $derived(
    languages.some(option => option.value === language) ? languages : [...languages, { value: language, label: language }],
  );
  const languageLabel = $derived(languageOptions.find(option => option.value === language)?.label ?? language);
  const remaining = $derived(Math.max(0, MAX_RECORDING_MS - elapsed));
  const lit = $derived(Math.round(level * 20));
  const lcdState = $derived(
    phase === "recording" ? "REC" : phase === "starting" ? "MIC…" : phase === "saving" ? "SAVING" : problem ? "NO MIC" : "READY",
  );
  const formatHint = $derived(
    willRecordAs === undefined ? "NO REC" : willRecordAs.startsWith("audio/mp4") ? "M4A 64K" : willRecordAs ? `${willRecordAs.split("/")[1]!.split(";")[0]!.toUpperCase()} 64K` : "64K",
  );

  function stamp(time: number) {
    const date = new Date(time);
    const pad = (value: number) => String(value).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}`;
  }
  function clock(ms: number) {
    const seconds = Math.floor(ms / 1000);
    return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
  }

  async function startRecording() {
    if (phase !== "idle") return;
    problem = null;
    player?.pause();
    phase = "starting";
    let started: Recording;
    try {
      started = await Recording.start(() => void keep("The microphone stopped, so the recording was kept."));
    } catch (error) {
      if (!destroyed) { problem = describeProblem(error); phase = "idle"; }
      return;
    }
    if (destroyed) { started.cancel(); return; }
    recording = started;
    elapsed = 0;
    phase = "recording";
    frame = requestAnimationFrame(meter);
  }
  function meter() {
    if (!recording) return;
    elapsed = recording.elapsed;
    level = recording.level();
    if (elapsed >= MAX_RECORDING_MS) { void keep("Five minutes is the most for one note, so it was kept."); return; }
    frame = requestAnimationFrame(meter);
  }
  function discard() {
    if (!recording) return;
    cancelAnimationFrame(frame);
    recording.cancel();
    recording = null;
    level = 0;
    elapsed = 0;
    phase = "idle";
    notify("Recording discarded. Nothing was saved.");
  }
  async function keep(reason?: string) {
    const taken = recording;
    if (phase !== "recording" || !taken) return;
    cancelAnimationFrame(frame);
    recording = null;
    phase = "saving";
    level = 0;
    const durationMs = Math.min(MAX_RECORDING_MS, Math.round(taken.elapsed));
    let blob: Blob;
    try {
      blob = await taken.stop();
    } catch (error) {
      problem = { kind: "failed", title: "The recording failed", detail: error instanceof Error ? error.message : "Try again." };
      phase = "idle";
      return;
    }
    if (blob.size === 0) {
      problem = { kind: "failed", title: "Nothing was recorded", detail: "The microphone sent no audio. Check the input in System Settings › Sound, then try again." };
      phase = "idle";
      return;
    }
    if (blob.size > MAX_ATTACHMENT_BYTES) {
      problem = { kind: "failed", title: "This recording is too large to keep", detail: `It is ${sizeLabel(blob.size)}; one note can hold at most 10 MB. Record a shorter note.` };
      phase = "idle";
      return;
    }
    const type = baseType(blob.type || taken.mimeType) || "audio/mp4";
    const file = new File([blob], `voice-note-${stamp(taken.startedAt)}.${extensionFor(type)}`, { type });
    let id = "";
    let clip: Clip | undefined;
    try {
      await attachments.import<typeof schema.descriptor>(file, (tx, ref) => {
        clip = { id: ref.id, byteLength: ref.byteLength, name: ref.name, mimeType: ref.mimeType };
        id = addRecording(tx, { recordedAt: new Date(taken.startedAt).toISOString(), durationMs, clip }).id;
      });
    } catch (error) {
      if (!destroyed) {
        problem = { kind: "failed", title: "The recording couldn't be saved", detail: error instanceof Error ? error.message : "Try again." };
        phase = "idle";
      }
      return;
    }
    if (destroyed) return;
    phase = "idle";
    elapsed = 0;
    notify(reason ?? `Recording kept (${durationLabel(durationMs)}).`);
    if (id && clip) void transcribe(id);
  }
  function toggleRecording() {
    if (phase === "idle") void startRecording();
    else if (phase === "recording") void keep();
  }

  function speechMessage(error: SpeechError): { global: boolean; tone: "unavailable" | "cancelled" | "failed"; text: string } {
    if (error.code === "cancelled")
      return { global: false, tone: "cancelled", text: "Transcription stopped because the window reloaded or an export started. Try again." };
    if (error.code === "failed")
      return { global: false, tone: "failed", text: `The recording couldn't be recognized. ${error.message}` };
    switch (error.reason) {
      case "host":
        return { global: true, tone: "unavailable", text: `${error.message}. Recordings still save, and you can type their text.` };
      case "system":
      case "permission":
      case "locale":
      case "notDeclared":
        return { global: true, tone: "unavailable", text: error.message };
      case "format":
        return { global: false, tone: "unavailable", text: error.message };
      default:
        return { global: false, tone: "unavailable", text: `Transcription unavailable. ${error.message}` };
    }
  }

  function ask(note: Note) {
    if (note.transcript.trim()) confirmReplace = note.$id;
    else void transcribe(note.$id);
  }
  async function transcribe(id: string) {
    const note = doc.current.notes.find(row => row.$id === id);
    if (!note?.clip || statuses[id]?.kind === "working") return;
    statuses[id] = { kind: "working" };
    try {
      const locale = doc.current.locale;
      const result = await speech.transcribe(note.clip.id, locale ? { locale } : undefined);
      speechNotice = null;
      if (!result.text.trim()) {
        statuses[id] = { kind: "problem", tone: "failed", text: "No words were recognized in this recording. The text was left as it was." };
        return;
      }
      await doc.change(tx => saveTranscription(tx, id, { text: result.text, locale: result.locale, at: new Date().toISOString() }));
      delete statuses[id];
      notify("Transcribed on-device.");
    } catch (error) {
      if (isSpeechError(error)) {
        const message = speechMessage(error);
        if (message.global) speechNotice = message.text;
        statuses[id] = { kind: "problem", tone: message.tone, text: message.global ? `Recording kept · ${sizeLabel(note.clip.byteLength)} · not transcribed` : message.text };
      } else if (isDocumentError(error) && !doc.current.notes.some(row => row.$id === id)) {
        delete statuses[id]; // The note was deleted during transcription.
      } else {
        statuses[id] = { kind: "problem", tone: "failed", text: error instanceof Error ? error.message : "Transcription failed." };
      }
    }
  }

  function togglePlay(note: Note) {
    if (!player || !note.clip) return;
    if (playingId === note.$id) {
      if (player.paused) void player.play().catch(playFailed);
      else player.pause();
      return;
    }
    playingId = note.$id;
    position = 0;
    player.src = attachments.url(note.clip.id);
    void player.play().catch(playFailed);
  }
  function playFailed(error: unknown) {
    if (error instanceof DOMException && error.name === "AbortError") return;
    if (playingId) statuses[playingId] = { kind: "problem", tone: "failed", text: "This recording can't be played in this window." };
    playingId = null;
  }

  async function typeNote() {
    await doc.change(tx => addTypedNote(tx, "", new Date().toISOString()));
    notify("Typed note added at the top. Click its line to write.");
  }
  async function remove(id: string) {
    confirmDelete = null;
    if (playingId === id) { player?.pause(); playingId = null; }
    await doc.change(tx => removeNote(tx, id));
    delete statuses[id];
    notify("Note deleted.");
  }
  async function chooseLanguage(value: string) {
    await doc.change(tx => setLocale(tx, value));
    speechNotice = null;
  }

  function onVisibility() {
    // Minimised or hidden: keep what was said and turn the microphone off.
    if (document.hidden && phase === "recording") void keep("The window was hidden, so the recording was stopped and kept.");
  }
  onMount(() => {
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    destroyed = true;
    cancelAnimationFrame(frame);
    recording?.cancel();
    recording = null;
    player?.pause();
  });
</script>

<main class="vn">
  <section class="vn-deck" aria-label="Recorder" data-phase={phase}>
    <div class="vn-deck-top">
      <span class="vn-brand">Voice&nbsp;Note</span>
      <span class="vn-grille" aria-hidden="true"></span>
      <span class="vn-model">VN-5 · on-device</span>
    </div>
    <div class="vn-deck-row">
      <div class="vn-rec-well">
        <button
          class="vn-rec"
          data-phase={phase}
          onclick={toggleRecording}
          aria-disabled={phase === "starting" || phase === "saving"}
          aria-label={phase === "recording" ? "Stop and keep the recording" : "Record a voice note"}
        >
          <svg class="vn-rec-ring" viewBox="0 0 100 100" aria-hidden="true">
            <circle cx="50" cy="50" r="46" pathLength="100" />
            {#if phase === "recording"}
              <circle class="vn-rec-progress" cx="50" cy="50" r="46" pathLength="100" stroke-dasharray={`${(elapsed / MAX_RECORDING_MS) * 100} 100`} />
            {/if}
          </svg>
          <span class="vn-rec-dome">{#if phase === "recording"}<i class="vn-rec-square"></i>{/if}</span>
        </button>
        <span class="vn-rec-caption">{phase === "recording" ? "Stop & keep" : phase === "starting" ? "Starting…" : phase === "saving" ? "Saving…" : "Record"}</span>
      </div>
      <div class="vn-lcd" role="status" aria-live="polite" aria-label="Recorder display">
        <div class="vn-lcd-line">
          <span class="vn-lcd-state" data-on={phase === "recording"}>{#if phase === "recording"}<i aria-hidden="true"></i>{/if}{lcdState}</span>
          <span class="vn-lcd-dim">{formatHint}</span>
        </div>
        <div class="vn-lcd-time">
          <span>{clock(elapsed)}</span><small>{phase === "recording" ? `−${clock(remaining)}` : "/05:00"}</small>
        </div>
        <div class="vn-meter" aria-hidden="true">
          {#each Array.from({ length: 20 }, (_, index) => index) as index (index)}
            <span data-lit={phase === "recording" && index < lit} data-hot={index >= 16}></span>
          {/each}
        </div>
      </div>
    </div>
    <div class="vn-keys">
      {#if phase === "recording"}
        <button class="vn-key vn-key-discard" onclick={discard}><X size={14} /> Discard</button>
        <span class="vn-key-note">Not saved until you stop.</span>
      {:else}
        <button class="vn-key" onclick={typeNote} disabled={phase !== "idle"}><PenLine size={14} /> Type a note</button>
        <Select.Root type="single" value={language} onValueChange={value => void chooseLanguage(value)} items={languageOptions}>
          <Select.Trigger class="vn-key vn-lang" aria-label={`Transcription language: ${languageLabel}`}>
            <span class="vn-lang-label">Transcribe in</span><span class="vn-lang-value">{languageLabel}</span><span aria-hidden="true">▾</span>
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="vn-options" sideOffset={6} align="end">
              <Select.Viewport>
                {#each languageOptions as option (option.value)}
                  <Select.Item class="vn-option" value={option.value} label={option.label}>{option.label}</Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      {/if}
    </div>
    {#if problem}
      <div class="vn-problem" role="alert">
        <div>
          <strong>{problem.title}</strong>
          <p>{problem.detail}</p>
        </div>
        <button class="vn-problem-close" aria-label="Dismiss" onclick={() => (problem = null)}><X size={15} /></button>
      </div>
    {/if}
  </section>

  <section class="vn-pad" aria-label="Notes">
    <Spiral />
    <div class="vn-pad-scroll">
      <header class="vn-pad-head">
        <EditableText class="vn-title" field={doc.fields.title} label="Pad title" placeholder="Name this pad" />
        <p class="vn-summary">
          {view.count} {view.count === 1 ? "note" : "notes"} · {durationLabel(view.recordedMs)} recorded · {view.words} words
        </p>
      </header>
      {#if speechNotice}
        <div class="vn-notice" role="status">
          <AudioLines size={16} aria-hidden="true" />
          <p>{speechNotice}</p>
          <button aria-label="Dismiss" onclick={() => (speechNotice = null)}><X size={14} /></button>
        </div>
      {/if}
      <ol class="vn-notes">
        {#each view.notes as note (note.$id)}
          {@const row = doc.at(note)}
          {@const status = statuses[note.$id]}
          {@const playing = playingId === note.$id}
          <li class="vn-note" data-playing={playing && !paused}>
            <div class="vn-margin">
              {#if note.clip}
                <button
                  class="vn-play"
                  onclick={() => togglePlay(note)}
                  aria-label={`${playing && !paused ? "Pause" : "Play"} recording, ${durationLabel(note.durationMs)}`}
                >{#if playing && !paused}<Pause size={16} />{:else}<Play size={16} />{/if}</button>
              {:else}
                <span class="vn-typed-mark" title="Typed note, no recording"><PenLine size={15} /></span>
              {/if}
            </div>
            <div class="vn-body">
              <div class="vn-note-head">
                <NoteMeta {note} />
                <div class="vn-actions">
                  {#if note.clip}
                    <button
                      class="vn-act"
                      onclick={() => ask(note)}
                      aria-disabled={status?.kind === "working"}
                    ><AudioLines size={14} /> {note.transcribed || note.transcript.trim() ? "Retranscribe" : "Transcribe"}</button>
                  {/if}
                  <button class="vn-act vn-act-icon" aria-label="Delete note" onclick={() => (confirmDelete = note.$id)}><Trash size={14} /></button>
                </div>
              </div>
              {#if playing}
                <div class="vn-progress" aria-hidden="true">
                  <span style:width={`${note.durationMs ? Math.min(100, (position * 1000 / note.durationMs) * 100) : 0}%`}></span>
                </div>
              {/if}
              <EditableText
                class="vn-transcript"
                field={row.transcript}
                label={`Text of the note from ${note.recordedAt.slice(0, 10)}`}
                placeholder={note.clip ? "No transcript yet. Click to type what was said." : "Click to write this note."}
              />
              {#if status?.kind === "working"}
                <p class="vn-status vn-status-working" role="status"><i aria-hidden="true"></i> Transcribing on-device…</p>
              {:else if status?.kind === "problem"}
                <p class="vn-status" data-tone={status.tone} role="status">{status.text}</p>
              {:else if note.transcribed}
                <p class="vn-status vn-status-quiet">Transcribed on-device · {note.transcribed.locale}{note.clip ? ` · ${sizeLabel(note.clip.byteLength)}` : ""}</p>
              {:else if note.clip}
                <p class="vn-status vn-status-quiet">Recording kept · {sizeLabel(note.clip.byteLength)}</p>
              {/if}
            </div>
          </li>
        {:else}
          <li class="vn-empty">
            <p>The pad is clear.</p>
            <p>Press the red button and say what's on your mind.</p>
          </li>
        {/each}
      </ol>
    </div>
  </section>

  <audio
    bind:this={player}
    bind:paused
    bind:currentTime={position}
    onended={() => (playingId = null)}
    onerror={() => playFailed(new Error("unplayable"))}
    preload="none"
  ></audio>

  <AlertDialog.Root open={confirmReplace !== null} onOpenChange={open => { if (!open) confirmReplace = null; }}>
    <AlertDialog.Portal>
      <AlertDialog.Overlay class="vn-overlay" />
      <AlertDialog.Content class="vn-dialog">
        <AlertDialog.Title class="vn-dialog-title">Replace this text?</AlertDialog.Title>
        <AlertDialog.Description class="vn-dialog-text">A new on-device transcription replaces the note's current text, including anything you corrected by hand.</AlertDialog.Description>
        <div class="vn-dialog-keys">
          <AlertDialog.Cancel class="vn-button">Keep my text</AlertDialog.Cancel>
          <AlertDialog.Action class="vn-button vn-button-strong" onclick={() => { const id = confirmReplace; confirmReplace = null; if (id) void transcribe(id); }}>Retranscribe</AlertDialog.Action>
        </div>
      </AlertDialog.Content>
    </AlertDialog.Portal>
  </AlertDialog.Root>

  <AlertDialog.Root open={confirmDelete !== null} onOpenChange={open => { if (!open) confirmDelete = null; }}>
    <AlertDialog.Portal>
      <AlertDialog.Overlay class="vn-overlay" />
      <AlertDialog.Content class="vn-dialog">
        <AlertDialog.Title class="vn-dialog-title">Delete this note?</AlertDialog.Title>
        <AlertDialog.Description class="vn-dialog-text">Its text and its recording are removed from this pad.</AlertDialog.Description>
        <div class="vn-dialog-keys">
          <AlertDialog.Cancel class="vn-button">Keep it</AlertDialog.Cancel>
          <AlertDialog.Action class="vn-button vn-button-danger" onclick={() => { if (confirmDelete) void remove(confirmDelete); }}>Delete</AlertDialog.Action>
        </div>
      </AlertDialog.Content>
    </AlertDialog.Portal>
  </AlertDialog.Root>
</main>
