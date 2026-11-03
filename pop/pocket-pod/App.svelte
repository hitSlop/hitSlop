<script lang="ts">
import "@hitslop/pop/base.css";
import "./styles.css";
import { onMount } from "svelte";
import { prefersReducedMotion } from "svelte/motion";
import { Button, Slider, Tabs } from "bits-ui";
import { attachments, bindText, getDocument, getEnvironment, haptics } from "@hitslop/pop/svelte";
import { refuse } from "@hitslop/pop";
import schema from "./schema";
import { createPod, menuItems } from "./controller.svelte";
import { closeClicker } from "./clicker";
import Background from "./Background.svelte";
import Breakout from "./Breakout.svelte";
import PodSheet from "./PodSheet.svelte";
import TrackArt from "./TrackArt.svelte";
import Case from "./Case.svelte";
import ClickWheel from "./ClickWheel.svelte";
import CoverFlow from "./CoverFlow.svelte";
import ListScreen, { type Row } from "./ListScreen.svelte";
import NowPlaying from "./NowPlaying.svelte";
import RemoveScreen from "./RemoveScreen.svelte";
import StickerGrid from "./StickerGrid.svelte";
import StickerLayer from "./StickerLayer.svelte";

const doc = getDocument(schema);
const pod = createPod();
const environment = getEnvironment();
const phone = $derived(environment.current.device === "phone");
const sheet = $derived(pod.ui.renaming ? "name" : pod.top.id === "add" ? "add" : pod.top.id === "customize" ? "customize" : null);
let skinBusy = $state(false);
let imagePicker: HTMLInputElement;
let songPicker: HTMLInputElement;
const backgroundURL = $derived(doc.current.background ? attachments.url(doc.current.background.id) : undefined);

function closeSheet() {
  if (pod.ui.renaming) pod.ui.renaming = false;
  else pod.dispatch({ type: "menu" });
}
async function importBackground(event: Event) {
  const field = event.currentTarget as HTMLInputElement;
  const file = field.files?.[0];
  field.value = "";
  if (!file) return;
  if ((!/^image\/(png|jpeg|webp|gif)$/.test(file.type) && !/\.(png|jpe?g|webp|gif)$/i.test(file.name)) || file.size > 10 * 1024 * 1024 || !file.size) {
    pod.flash("Choose a PNG, JPEG, WebP or GIF up to 10 MB.", "error");
    return;
  }
  skinBusy = true;
  try {
    await attachments.import<typeof schema.descriptor>(file, (tx, ref) => {
      if (!/^image\/(png|jpeg|webp|gif)$/.test(ref.mimeType)) refuse("Choose a PNG, JPEG, WebP or GIF image.");
      tx.fields.background.$set({ id: ref.id, name: ref.name, mimeType: ref.mimeType });
    });
    pod.flash("Photo skin applied.", "ok");
    haptics.notification("success");
  } finally { skinBusy = false; }
}
async function importSongs(event: Event) {
  const field = event.currentTarget as HTMLInputElement;
  const files = Array.from(field.files ?? []);
  field.value = "";
  await pod.importSongs(files);
}
function stickerPack() {
  closeSheet();
  pod.push("stickers");
}

let stage = $state<HTMLElement>();
let stageWidth = $state(360);
let stageHeight = $state(603);
const scale = $derived(Math.max(0.1, Math.min(stageWidth / 360, stageHeight / 603)));
let booting = $state(false);

const top = $derived(pod.top);
const playing = $derived(pod.playback.started && pod.playback.state === 1);
const paused = $derived(pod.playback.started && pod.playback.state === 2);
const held = $derived(pod.ui.held);

const menuRows: Row[] = menuItems.map((label) => ({ label, chevron: label !== "Shuffle" }));
const stickerRows: Row[] = [{ label: "Add Sticker", chevron: true }, { label: "Remove Last" }, { label: "Clear All" }];
const videoRows = $derived<Row[]>(pod.videos.map((video) => ({ label: video.title })));
const settingsRows = $derived<Row[]>([
  { label: "Clicker", value: doc.current.clicker ? "On" : "Off" },
  { label: "Repeat", value: { off: "Off", all: "All", one: "One" }[doc.current.repeat] },
  { label: "Shuffle", value: doc.current.shuffle ? "On" : "Off" },
  { label: "Name", value: doc.current.ownerName.trim() || "Not set" },
]);
const selectedSticker = $derived(pod.ui.selectedSticker);

// Pointing at a row moves the cursor; pointing at the row under the cursor chooses it.
const activate = (index: number) => {
  if (phone) { pod.setCursor(index); pod.dispatch({ type: "select" }); }
  else if (index === pod.top.cursor) pod.dispatch({ type: "select" });
  else pod.setCursor(index);
};
const editable = (target: EventTarget | null) => target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;

function onKey(event: KeyboardEvent): void {
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  booting = false;
  const target = event.target as HTMLElement | null;
  if (target?.closest("[data-dialog-content]")) return;
  if (target?.closest(".pod-sticker")) return;
  if (editable(target)) {
    if (event.key === "Escape") { event.preventDefault(); pod.dispatch({ type: "menu" }); }
    return;
  }
  // A button reached with Tab keeps Enter and Space; one the mouse clicked does not.
  const onButton = target instanceof HTMLButtonElement && target.matches(":focus-visible");
  const intent = {
    ArrowUp: { type: "scroll", delta: -1 }, ArrowDown: { type: "scroll", delta: 1 },
    ArrowLeft: { type: "prev" }, ArrowRight: { type: "next" },
    Escape: { type: "menu" }, Backspace: { type: "menu" },
    Enter: onButton ? undefined : { type: "select" }, " ": onButton ? undefined : { type: "playpause" },
  }[event.key] as Parameters<typeof pod.dispatch>[0] | undefined;
  if (!intent) return;
  event.preventDefault();
  pod.dispatch(intent);
}

function onPaste(event: ClipboardEvent): void {
  if (editable(event.target) || sheet) return;
  const text = event.clipboardData?.getData("text") ?? "";
  if (!text.trim()) return;
  event.preventDefault();
  void pod.addFromText(text);
}

// A mouse or trackpad over the pod turns the wheel: one detent per 40 units of travel.
let wheelCarry = 0;
function onWheel(event: WheelEvent): void {
  if (phone && (event.target as HTMLElement).closest(".pod-rows")) return;
  event.preventDefault();
  booting = false;
  wheelCarry += Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
  while (Math.abs(wheelCarry) >= 40) {
    const delta = wheelCarry > 0 ? 1 : -1;
    pod.dispatch({ type: "scroll", delta });
    wheelCarry -= delta * 40;
  }
}

// The mouse has no use for focus on a button; releasing it keeps the pod's keys working.
function onStagePointerUp(event: PointerEvent): void {
  if (event.pointerType === "mouse") (event.target as HTMLElement).closest("button")?.blur();
}

function onStagePointerDown(event: PointerEvent): void {
  booting = false;
  if (!(event.target as HTMLElement).closest(".pod-sticker, .pod-tools, button, input")) pod.ui.selectedSticker = null;
}

onMount(() => {
  let bootTimer: ReturnType<typeof setTimeout> | undefined;
  if (!prefersReducedMotion.current) {
    booting = true;
    bootTimer = setTimeout(() => { booting = false; }, 1500);
  }
  window.addEventListener("keydown", onKey);
  window.addEventListener("paste", onPaste);
  stage?.addEventListener("wheel", onWheel, { passive: false });
  return () => {
    clearTimeout(bootTimer);
    window.removeEventListener("keydown", onKey);
    window.removeEventListener("paste", onPaste);
    stage?.removeEventListener("wheel", onWheel);
    pod.dispose();
    closeClicker();
  };
});
</script>

{#snippet menuAside()}
  {#if pod.current}
    <TrackArt class="pod-aside-art" video={pod.current} />
    <p class="pod-aside-title">{pod.current.title}</p>
    <p class="pod-aside-sub">{pod.current.author}</p>
  {:else}
    <div class="pod-aside-art pod-aside-empty" aria-hidden="true">+</div>
    <p class="pod-aside-title">Add songs or a YouTube link</p>
  {/if}
{/snippet}

{#snippet stickers()}
  <StickerLayer
    stickers={doc.current.stickers}
    selectedId={selectedSticker}
    interactive
    onselect={(id) => { pod.ui.selectedSticker = id; }}
    oncommit={pod.placeSticker}
    onpeel={pod.peelSticker}
    onturn={pod.turnSticker}
    onsize={pod.sizeSticker}
  />
{/snippet}

{#snippet screen()}
  <div class="pod-layer" inert={top.id !== "playing"} aria-hidden={top.id !== "playing"}>
    <NowPlaying
      video={pod.current}
      playback={pod.playback}
      scrub={pod.ui.scrub}
      volume={pod.ui.volume ?? doc.current.volume}
      volumeVisible={pod.ui.volumeVisible}
      systemVolume={pod.systemVolume}
      {held}
      handlers={pod.handlers}
      onattach={pod.attach}
    />
  </div>
  {#if top.id !== "playing"}
    <div class="pod-layer pod-layer-front">
      {#if (top.id === "menu" || top.id === "add" || top.id === "customize")}
        <ListScreen touch={phone} title={pod.menuTitle} rows={menuRows} cursor={top.cursor} aside={menuAside} {playing} {paused} {held} onrow={activate} />
      {:else if top.id === "coverflow"}
        <CoverFlow videos={pod.videos} cursor={top.cursor} {playing} {paused} {held} onpick={activate} />
      {:else if top.id === "videos"}
        <ListScreen
          touch={phone}
          title="Songs"
          rows={videoRows}
          cursor={top.cursor}
          footer={pod.videos[top.cursor]?.author ?? ""}
          empty="No songs yet. Add local audio or a YouTube link."
          {playing}
          {paused}
          {held}
          onrow={activate}
        />
      {:else if top.id === "settings"}
        <ListScreen touch={phone} title="Settings" rows={settingsRows} cursor={top.cursor} {playing} {paused} {held} onrow={activate} />
      {:else if top.id === "games"}
        <ListScreen touch={phone} title="Games" rows={[{ label: "Brick Breaker", chevron: true }]} cursor={0} footer={`Best: ${doc.current.brickBest} · Your music keeps playing`} {playing} {paused} {held} onrow={activate} />
      {:else if top.id === "breakout"}
        <Breakout {held} {playing} {paused} best={doc.current.brickBest} onbest={(score) => doc.fields.brickBest.set(score)} onattach={pod.attachGame} />
      {:else if top.id === "stickers"}
        <ListScreen
          touch={phone}
          title="Stickers"
          rows={stickerRows}
          cursor={top.cursor}
          footer={`${doc.current.stickers.length} on your Pod`}
          {playing}
          {paused}
          {held}
          onrow={activate}
        />
      {:else if top.id === "stickerPack"}
        <StickerGrid cursor={top.cursor} {playing} {paused} {held} onpick={activate} />
      {:else if top.id === "remove"}
        <RemoveScreen video={pod.videos.find((video) => video.$id === pod.ui.removeId)} {playing} {paused} {held} />
      {/if}
    </div>
  {/if}
  <div class="pod-toast-region" role="status" aria-live="polite">
    {#if pod.ui.notice}<p class="pod-toast" data-tone={pod.ui.notice.tone}>{pod.ui.notice.text}</p>{/if}
  </div>
  {#if booting}
    <div class="pod-boot" aria-hidden="true">
      <svg class="pod-boot-face" viewBox="0 0 48 64">
        <rect x="3" y="3" width="42" height="58" rx="9" fill="none" stroke="currentColor" stroke-width="3" />
        <rect x="9" y="9" width="30" height="22" rx="3" fill="currentColor" opacity=".16" />
        <circle cx="19" cy="17" r="2" fill="currentColor" /><circle cx="29" cy="17" r="2" fill="currentColor" />
        <path d="M17 23 Q24 29 31 23" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" />
        <circle cx="24" cy="46" r="10" fill="none" stroke="currentColor" stroke-width="2.4" /><circle cx="24" cy="46" r="3.4" fill="currentColor" />
      </svg>
      <strong>{pod.menuTitle}</strong>
      <span class="pod-boot-track"><i></i></span>
    </div>
  {/if}
{/snippet}

{#snippet tray()}
  <div class="pod-quick-actions">
    <Button.Root class="pod-quick" disabled={held} onclick={() => pod.push("customize")} aria-label="Customize your Pod">✦ <span>Customize</span></Button.Root>
    <span class="pod-deboss">POCKET POD</span>
    <Button.Root class="pod-quick" disabled={held} onclick={() => pod.push("add")}>+ <span>Add music</span></Button.Root>
  </div>
{/snippet}

{#snippet dock()}
  {#if selectedSticker}
    <div class="pod-tools" role="toolbar" aria-label="Sticker tools">
      <Button.Root class="pod-tool" aria-label="Turn left" onclick={() => pod.turnSticker(selectedSticker, -15)}>↺</Button.Root>
      <Button.Root class="pod-tool" aria-label="Turn right" onclick={() => pod.turnSticker(selectedSticker, 15)}>↻</Button.Root>
      <Button.Root class="pod-tool" aria-label="Smaller" onclick={() => pod.sizeSticker(selectedSticker, 1 / 1.1)}>−</Button.Root>
      <Button.Root class="pod-tool" aria-label="Bigger" onclick={() => pod.sizeSticker(selectedSticker, 1.1)}>+</Button.Root>
      <Button.Root class="pod-tool pod-tool-peel" aria-label="Peel sticker off" onclick={() => pod.peelSticker(selectedSticker)}>Peel</Button.Root>
    </div>
  {/if}
{/snippet}

{#snippet wheel()}
  <ClickWheel {held} ondispatch={pod.dispatch} />
{/snippet}

{#snippet hold()}
  <Button.Root class="pod-hold" aria-label="Hold switch" aria-pressed={held} onclick={() => { pod.ui.held = !pod.ui.held; }}>
    <span class="pod-hold-slot"><i></i></span>
    <span class="pod-hold-label">HOLD</span>
  </Button.Root>
{/snippet}

<main
  class="pod-stage pod-surface"
  data-phone={phone}
  bind:this={stage}
  bind:clientWidth={stageWidth}
  bind:clientHeight={stageHeight}
  aria-label={pod.menuTitle}
  onpointerdown={onStagePointerDown}
  onpointerup={onStagePointerUp}
>
  <Background source={backgroundURL} fit={doc.current.backgroundFit} dim={doc.current.backgroundDim} />
  {#if phone}<header class="pod-phone-heading"><strong>{pod.menuTitle}</strong></header>{/if}
  <div class="pod-fit" style:transform={phone ? undefined : `translate(-50%, -50%) scale(${scale})`}>
    <Case {stickers} {screen} {tray} {wheel} {hold} {dock} />
  </div>
</main>


<input class="pod-file-input" bind:this={imagePicker} type="file" accept="image/png,image/jpeg,image/webp,image/gif" aria-label="Upload background image" onchange={importBackground} />
<input class="pod-file-input" bind:this={songPicker} type="file" accept="audio/mpeg,audio/mp4,audio/wav,audio/flac,audio/ogg,.mp3,.m4a,.wav,.flac,.ogg,.opus" multiple aria-label="Import local songs" onchange={importSongs} />

<PodSheet open={sheet !== null} title={sheet === "add" ? "Add music" : sheet === "name" ? "Name your Pod" : "Make it yours"}
  description={sheet === "add" ? "Local favorites and YouTube, together on your Pod." : sheet === "name" ? "A little player with your name on it." : "Your photos, your colors, your stickers."} onclose={closeSheet}>
  {#if pod.ui.notice}<p class="pod-sheet-message" role="status">{pod.ui.notice.text}</p>{/if}
  {#if sheet === "add"}
    <Button.Root class="pod-sheet-action" disabled={pod.ui.busy} onclick={() => songPicker.click()}>
      <span aria-hidden="true">♫</span><span>{pod.ui.busy ? "Adding music…" : "Choose audio files"}<small>From Files · up to 10 MB per song</small></span><span aria-hidden="true">＋</span>
    </Button.Root>
    <p class="pod-sheet-help">MP3, M4A and WAV work best on iPhone. Imported files travel with this pop and play offline. Subscription downloads from music apps cannot be imported.</p>
    <form class="pod-sheet-form" onsubmit={(event) => { event.preventDefault(); void pod.submitAdd(); }}>
      <label for="pod-add-link">Or paste a YouTube link</label>
      <input id="pod-add-link" bind:value={pod.ui.addText} type="url" inputmode="url" autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="youtube.com/watch?v=…" disabled={pod.ui.busy} />
      <Button.Root type="submit" class="pod-sheet-primary" disabled={pod.ui.busy || !pod.ui.addText.trim()}>{pod.ui.busy ? "Adding…" : "Add YouTube video"}</Button.Root>
    </form>
    <p class="pod-sheet-help">YouTube needs an internet connection. Open Songs, choose a track, then press play. Hold the center button on a song to remove it.</p>
  {:else if sheet === "name"}
    <div class="pod-sheet-form">
      <label for="pod-name">Your name</label>
      <input id="pod-name" use:bindText={doc.fields.ownerName} maxlength="24" autocomplete="off" placeholder="Your name" onkeydown={(event) => { if (event.key === "Enter") { event.preventDefault(); closeSheet(); } }} />
      <Button.Root class="pod-sheet-primary" onclick={closeSheet}>Done</Button.Root>
    </div>
  {:else if sheet === "customize"}
    <div class="pod-skin-preview" style:background-image={backgroundURL ? `url("${backgroundURL}")` : undefined} style:background-size={doc.current.backgroundFit}>
      <span>{doc.current.background ? doc.current.background.name : "A blank canvas for your soundtrack"}</span>
    </div>
    <Button.Root class="pod-sheet-action" disabled={skinBusy} onclick={() => imagePicker.click()}><span aria-hidden="true">▧</span><span>{skinBusy ? "Importing skin…" : doc.current.background ? "Change photo skin" : "Upload a photo skin"}<small>PNG, JPEG, WebP or GIF · up to 10 MB</small></span><span aria-hidden="true">＋</span></Button.Root>
    {#if doc.current.background}
      <Tabs.Root value={doc.current.backgroundFit} onValueChange={(value) => { if (value === "cover" || value === "contain") void doc.fields.backgroundFit.set(value); }}>
        <Tabs.List class="pod-fit-options" aria-label="Photo fit"><Tabs.Trigger value="cover">Fill the case</Tabs.Trigger><Tabs.Trigger value="contain">Show whole photo</Tabs.Trigger></Tabs.List>
      </Tabs.Root>
      <div class="pod-skin-dim">
        <label for="pod-dim">Shade <span>{Math.round(doc.current.backgroundDim * 100)}%</span></label>
        <Slider.Root id="pod-dim" type="single" min={0} max={0.8} step={0.05} value={doc.current.backgroundDim} onValueChange={(value) => doc.fields.backgroundDim.preview(value)} onValueCommit={(value) => { void doc.fields.backgroundDim.set(value); }} aria-label="Background shade" class="pod-dim-slider">
          {#snippet children({ thumbs })}<span class="pod-dim-track"><Slider.Range class="pod-dim-range" /></span>{#each thumbs as thumb}<Slider.Thumb index={thumb} class="pod-dim-thumb" />{/each}{/snippet}
        </Slider.Root>
      </div>
      <Button.Root class="pod-sheet-text" onclick={() => doc.fields.background.$clear()}>Remove photo skin</Button.Root>
    {/if}
    <Button.Root class="pod-sheet-action" onclick={stickerPack}><span aria-hidden="true">✦</span><span>Sticker collection<small>Add, drag, turn and resize · {doc.current.stickers.length} on your Pod</small></span><span aria-hidden="true">›</span></Button.Root>
    <p class="pod-sheet-help">Change the case, wheel, button and screen colors in the pop’s theme palette. Give it a name in Settings → Name.</p>
  {/if}
</PodSheet>
