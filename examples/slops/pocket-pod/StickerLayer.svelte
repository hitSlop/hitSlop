<script lang="ts">
import Sticker from "./Sticker.svelte";
import type { Sticker as StickerRow } from "./schema";

let { stickers, selectedId = null, interactive = false, onselect, oncommit, onpeel, onturn, onsize }: {
  stickers: readonly StickerRow[];
  selectedId?: string | null;
  interactive?: boolean;
  onselect?: (id: string | null) => void;
  oncommit?: (id: string, x: number, y: number) => Promise<void> | void;
  onpeel?: (id: string) => void;
  onturn?: (id: string, degrees: number) => void;
  onsize?: (id: string, factor: number) => void;
} = $props();

const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
let layer: HTMLDivElement;
let drag = $state.raw<{ id: string; x: number; y: number } | null>(null);
let grab: { id: string; pointer: number; dx: number; dy: number; startX: number; startY: number } | null = null;

function fraction(event: PointerEvent) {
  const box = layer.getBoundingClientRect();
  return { x: (event.clientX - box.left) / box.width, y: (event.clientY - box.top) / box.height };
}
function down(event: PointerEvent, sticker: StickerRow) {
  if (!interactive || event.button !== 0) return;
  const at = fraction(event);
  grab = { id: sticker.$id, pointer: event.pointerId, dx: sticker.x - at.x, dy: sticker.y - at.y, startX: event.clientX, startY: event.clientY };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  onselect?.(sticker.$id);
}
function move(event: PointerEvent) {
  if (!grab || event.pointerId !== grab.pointer) return;
  if (!drag && Math.hypot(event.clientX - grab.startX, event.clientY - grab.startY) < 3) return;
  const at = fraction(event);
  drag = { id: grab.id, x: clamp(at.x + grab.dx, 0, 1), y: clamp(at.y + grab.dy, 0, 1) };
}
function up(event: PointerEvent) {
  if (!grab || event.pointerId !== grab.pointer) return;
  grab = null;
  // A pointer user hasn't asked for keyboard control of the sticker; give the keys back to the pod.
  if (event.pointerType === "mouse") (event.currentTarget as HTMLElement).blur();
  const final = drag;
  // Keep showing the dragged position until the document has accepted it.
  if (final) void Promise.resolve(oncommit?.(final.id, final.x, final.y)).finally(() => { if (drag === final) drag = null; });
}
function key(event: KeyboardEvent, sticker: StickerRow) {
  const nudge = (dx: number, dy: number) => void oncommit?.(sticker.$id, clamp(sticker.x + dx, 0, 1), clamp(sticker.y + dy, 0, 1));
  const handled = {
    ArrowLeft: () => nudge(-0.012, 0), ArrowRight: () => nudge(0.012, 0),
    ArrowUp: () => nudge(0, -0.008), ArrowDown: () => nudge(0, 0.008),
    "[": () => onturn?.(sticker.$id, -15), "]": () => onturn?.(sticker.$id, 15),
    "-": () => onsize?.(sticker.$id, 1 / 1.1), "=": () => onsize?.(sticker.$id, 1.1), "+": () => onsize?.(sticker.$id, 1.1),
    Delete: () => onpeel?.(sticker.$id), Backspace: () => onpeel?.(sticker.$id),
    Escape: () => onselect?.(null),
    Enter: () => onselect?.(sticker.$id),
  }[event.key];
  if (!handled || event.metaKey || event.ctrlKey || event.altKey) return;
  event.preventDefault();
  event.stopPropagation();
  handled();
}
</script>

<div class="pod-stickers" bind:this={layer}>
  {#each stickers as sticker (sticker.$id)}
    {@const at = drag?.id === sticker.$id ? drag : sticker}
    <div
      class="pod-sticker"
      {...interactive ? {
        role: "button",
        tabindex: 0,
        "aria-label": `${sticker.kind} sticker. Arrow keys move it, brackets turn it, minus and plus resize it, Delete peels it off.`,
        "aria-pressed": selectedId === sticker.$id,
      } : {}}
      data-selected={selectedId === sticker.$id}
      data-dragging={drag?.id === sticker.$id}
      style:left="{at.x * 360}px"
      style:top="{at.y * 603}px"
      style:--turn="{sticker.rotation}deg"
      style:--grow={sticker.scale}
      onpointerdown={(event) => down(event, sticker)}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
      onkeydown={(event) => key(event, sticker)}
    >
      <Sticker kind={sticker.kind} />
    </div>
  {/each}
</div>
