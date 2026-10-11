<script lang="ts">
import { Button } from "bits-ui";
import type { Intent } from "./intents";

let { held, ondispatch }: { held: boolean; ondispatch: (intent: Intent) => void } = $props();

type Sector = "menu" | "next" | "playpause" | "prev";
const DETENT = Math.PI / 12;
const PRESS_SLOP = 0.14;
const HOLD_MS = 450;
const SEEK_MS = 260;

let ring: HTMLDivElement;
let pressed = $state<Sector | "select" | null>(null);
let gesture: { id: number; last: number; total: number; carry: number; sector: Sector; seeking: boolean } | null = null;
let holdTimer: ReturnType<typeof setTimeout> | undefined;
let seekTimer: ReturnType<typeof setInterval> | undefined;
let centerTimer: ReturnType<typeof setTimeout> | undefined;
let centerHeld = false;

function angleAt(event: PointerEvent): number {
  const box = ring.getBoundingClientRect();
  return Math.atan2(event.clientY - (box.top + box.height / 2), event.clientX - (box.left + box.width / 2));
}
function sectorAt(angle: number): Sector {
  const degrees = (angle * 180) / Math.PI;
  if (degrees >= -135 && degrees < -45) return "menu";
  if (degrees >= -45 && degrees < 45) return "next";
  if (degrees >= 45 && degrees < 135) return "playpause";
  return "prev";
}
function stopTimers(): void {
  clearTimeout(holdTimer);
  clearInterval(seekTimer);
}

function down(event: PointerEvent): void {
  if (held || event.button !== 0) return;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  const angle = angleAt(event);
  const sector = sectorAt(angle);
  gesture = { id: event.pointerId, last: angle, total: 0, carry: 0, sector, seeking: false };
  pressed = sector;
  if (sector === "prev" || sector === "next") {
    holdTimer = setTimeout(() => {
      if (!gesture || gesture.total > PRESS_SLOP) return;
      gesture.seeking = true;
      const delta = sector === "next" ? 1 : -1;
      ondispatch({ type: "seek", delta });
      seekTimer = setInterval(() => ondispatch({ type: "seek", delta }), SEEK_MS);
    }, HOLD_MS);
  }
}

function move(event: PointerEvent): void {
  if (!gesture || event.pointerId !== gesture.id) return;
  const angle = angleAt(event);
  let delta = angle - gesture.last;
  if (delta > Math.PI) delta -= 2 * Math.PI;
  if (delta < -Math.PI) delta += 2 * Math.PI;
  gesture.last = angle;
  gesture.total += Math.abs(delta);
  if (gesture.total > PRESS_SLOP) { stopTimers(); pressed = null; }
  if (gesture.seeking) return;
  gesture.carry += delta;
  while (Math.abs(gesture.carry) >= DETENT) {
    const direction = gesture.carry > 0 ? 1 : -1;
    ondispatch({ type: "scroll", delta: direction });
    gesture.carry -= direction * DETENT;
  }
}

function up(event: PointerEvent): void {
  if (!gesture || event.pointerId !== gesture.id) return;
  stopTimers();
  const { total, seeking, sector } = gesture;
  gesture = null;
  pressed = null;
  if (event.type === "pointercancel" || seeking || total > PRESS_SLOP) return;
  ondispatch({ type: sector });
}

function centerDown(): void {
  if (held) return;
  centerHeld = false;
  pressed = "select";
  centerTimer = setTimeout(() => { centerHeld = true; ondispatch({ type: "hold-select" }); }, 600);
}
function centerUp(): void {
  clearTimeout(centerTimer);
  pressed = null;
}
function centerClick(): void {
  if (centerHeld) { centerHeld = false; return; }
  // Pointer clicks and keyboard activation both land here.
  ondispatch({ type: "select" });
}
const key = (type: Sector) => (event: MouseEvent) => {
  // Pointer presses are handled by the gesture layer above; this serves keyboard activation.
  if (event.detail === 0) ondispatch({ type });
};
</script>

<div class="pod-wheel" bind:this={ring} role="group" aria-label="Click wheel" data-held={held} data-pressed={pressed}>
  <Button.Root class="pod-wheel-label pod-wheel-menu" onclick={key("menu")} aria-label="Menu">MENU</Button.Root>
  <Button.Root class="pod-wheel-label pod-wheel-prev" onclick={key("prev")} aria-label="Previous video">
    <svg viewBox="0 0 24 14" aria-hidden="true"><path d="M2 1v12M13 1L4 7l9 6zM22 1l-9 6 9 6z" /></svg>
  </Button.Root>
  <Button.Root class="pod-wheel-label pod-wheel-next" onclick={key("next")} aria-label="Next video">
    <svg viewBox="0 0 24 14" aria-hidden="true"><path d="M22 1v12M11 1l9 6-9 6zM2 1l9 6-9 6z" /></svg>
  </Button.Root>
  <Button.Root class="pod-wheel-label pod-wheel-play" onclick={key("playpause")} aria-label="Play or pause">
    <svg viewBox="0 0 26 14" aria-hidden="true"><path d="M1 1l10 6-10 6zM16 1h3.5v12H16zM22 1h3.5v12H22z" /></svg>
  </Button.Root>
  <div
    class="pod-wheel-grip"
    role="presentation"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
  ></div>
  <Button.Root
    class="pod-wheel-center"
    aria-label="Select"
    onpointerdown={centerDown}
    onpointerup={centerUp}
    onpointercancel={centerUp}
    onpointerleave={centerUp}
    onclick={centerClick}
  />
</div>
