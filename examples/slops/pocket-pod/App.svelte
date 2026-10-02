<script lang="ts">
import { onMount } from "svelte";
import { prefersReducedMotion } from "svelte/motion";
import { Button } from "bits-ui";
import { bindText } from "@hitslop/document/svelte";
import { youtube } from "@hitslop/document/embed";
import doc from "./schema";
import { createPod, menuItems, stickerItems } from "./pod.svelte";
import { closeClicker } from "./clicker";
import AddScreen from "./AddScreen.svelte";
import Case from "./Case.svelte";
import ClickWheel from "./ClickWheel.svelte";
import CoverFlow from "./CoverFlow.svelte";
import ListScreen, { type Row } from "./ListScreen.svelte";
import NowPlaying from "./NowPlaying.svelte";
import RemoveScreen from "./RemoveScreen.svelte";
import StickerGrid from "./StickerGrid.svelte";
import StickerLayer from "./StickerLayer.svelte";

const pod = createPod();

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
  if (index === pod.top.cursor) pod.dispatch({ type: "select" });
  else pod.setCursor(index);
};
const focusOnMount = (node: HTMLInputElement) => node.focus();
const editable = (target: EventTarget | null) => target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;

function onKey(event: KeyboardEvent): void {
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  booting = false;
  const target = event.target as HTMLElement | null;
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
  if (editable(event.target)) return;
  const text = event.clipboardData?.getData("text") ?? "";
  if (!text.trim()) return;
  event.preventDefault();
  void pod.addFromText(text);
}

// A mouse or trackpad over the pod turns the wheel: one detent per 40 units of travel.
let wheelCarry = 0;
function onWheel(event: WheelEvent): void {
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
    <img class="pod-aside-art" src={youtube.thumbnail(pod.current.youtubeId)} alt="" draggable="false" />
    <p class="pod-aside-title">{pod.current.title}</p>
    <p class="pod-aside-sub">{pod.current.author}</p>
  {:else}
    <div class="pod-aside-art pod-aside-empty" aria-hidden="true">+</div>
    <p class="pod-aside-title">Paste a YouTube link to add a video</p>
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
      {held}
      handlers={pod.handlers}
      onattach={pod.attach}
    />
  </div>
  {#if top.id !== "playing"}
    <div class="pod-layer pod-layer-front">
      {#if top.id === "menu"}
        <ListScreen title={pod.menuTitle} rows={menuRows} cursor={top.cursor} aside={menuAside} {playing} {paused} {held} onrow={activate} />
      {:else if top.id === "coverflow"}
        <CoverFlow videos={pod.videos} cursor={top.cursor} {playing} {paused} {held} onpick={activate} />
      {:else if top.id === "videos"}
        <ListScreen
          title="Videos"
          rows={videoRows}
          cursor={top.cursor}
          footer={pod.videos[top.cursor]?.author ?? ""}
          empty="No videos yet. Paste a YouTube link."
          {playing}
          {paused}
          {held}
          onrow={activate}
        />
      {:else if top.id === "add"}
        <AddScreen
          bind:text={pod.ui.addText}
          busy={pod.ui.busy}
          {playing}
          {paused}
          {held}
          onsubmit={() => pod.dispatch({ type: "select" })}
          onescape={() => pod.dispatch({ type: "menu" })}
        />
      {:else if top.id === "settings"}
        <ListScreen title="Settings" rows={settingsRows} cursor={top.cursor} {playing} {paused} {held} onrow={activate} />
        {#if pod.ui.renaming}
          <div class="pod-rename">
            <label for="pod-name">Your name</label>
            <input
              id="pod-name"
              use:focusOnMount
              use:bindText={doc.fields.ownerName}
              maxlength="24"
              autocomplete="off"
              spellcheck="false"
              onkeydown={(event) => { if (event.key === "Enter" || event.key === "Escape") { event.preventDefault(); event.stopPropagation(); pod.ui.renaming = false; } }}
            />
          </div>
        {/if}
      {:else if top.id === "stickers"}
        <ListScreen
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
  <span class="pod-deboss">POCKET POD</span>
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
  bind:this={stage}
  bind:clientWidth={stageWidth}
  bind:clientHeight={stageHeight}
  aria-label={pod.menuTitle}
  onpointerdown={onStagePointerDown}
  onpointerup={onStagePointerUp}
>
  <div class="pod-fit" style:transform="translate(-50%, -50%) scale({scale})">
    <Case {stickers} {screen} {tray} {wheel} {hold} {dock} />
  </div>
</main>
