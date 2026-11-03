<script lang="ts">
import { onMount, untrack } from "svelte";
import { Button } from "bits-ui";
import { haptics } from "@hitslop/pop/svelte";
import TitleBar from "./TitleBar.svelte";
import type { Intent } from "./intents";

let { held, playing, paused, best, onbest, onattach }: {
  held: boolean;
  playing: boolean;
  paused: boolean;
  best: number;
  onbest: (score: number) => Promise<void>;
  onattach: (handler: ((intent: Intent) => void) | null) => void;
} = $props();

// The playfield has a fixed coordinate system; the SVG fits either screen size.
// Only a completed personal best is a document edit. Physics stays on this page.
const W = 300, H = 152, PADDLE = 52, RADIUS = 3, PADDLE_Y = 136;
const newBricks = () => Array.from({ length: 32 }, (_, id) => ({ id, x: 10 + (id % 8) * 35.5, y: 13 + Math.floor(id / 8) * 13 }));
let bricks = $state(newBricks());
let paddle = $state(W / 2);
let ball = $state({ x: W / 2, y: PADDLE_Y - 5 });
let score = $state(0);
let lives = $state(3);
let level = $state(1);
let status = $state<"ready" | "playing" | "paused" | "over">("ready");
let velocity = { x: 62, y: -100 };
let arena: SVGSVGElement;
let frame = 0;
let lastTime = 0;
let recordedBest = untrack(() => best);
let visible = $state(true);
let dragPointer: number | null = null;
let dragStart = { x: 0, y: 0 };
let moved = false;

const clamp = (x: number) => Math.max(PADDLE / 2, Math.min(W - PADDLE / 2, x));
function movePaddle(x: number) {
  if (held || status === "over") return;
  paddle = clamp(x);
  if (status === "ready") ball = { x: paddle, y: PADDLE_Y - 5 };
}
async function recordBest() {
  if (score <= Math.max(best, recordedBest)) return;
  const next = score;
  await onbest(next);
  recordedBest = Math.max(recordedBest, next);
}
function restart() {
  score = 0; lives = 3; level = 1; bricks = newBricks(); paddle = W / 2;
  ball = { x: paddle, y: PADDLE_Y - 5 }; status = "ready";
}
function launch() {
  if (held || !visible) return;
  if (status === "over") { restart(); return; }
  if (status === "playing") { status = "paused"; return; }
  if (status === "ready") velocity = { x: 62 + (level - 1) * 8, y: -100 - (level - 1) * 12 };
  status = "playing";
  lastTime = 0;
}
function intent(action: Intent) {
  if (action.type === "scroll") movePaddle(paddle + action.delta * 14);
  else if (action.type === "prev") movePaddle(paddle - 22);
  else if (action.type === "next") movePaddle(paddle + 22);
  else if (action.type === "select") launch();
  else if (action.type === "menu") void recordBest();
}
function step(dt: number) {
  const previous = { ...ball };
  ball.x += velocity.x * dt;
  ball.y += velocity.y * dt;
  if (ball.x < RADIUS || ball.x > W - RADIUS) {
    ball.x = Math.max(RADIUS, Math.min(W - RADIUS, ball.x));
    velocity.x *= -1;
  }
  if (ball.y < RADIUS) { ball.y = RADIUS; velocity.y = Math.abs(velocity.y); }
  if (velocity.y > 0 && previous.y + RADIUS <= PADDLE_Y && ball.y + RADIUS >= PADDLE_Y && Math.abs(ball.x - paddle) <= PADDLE / 2 + RADIUS) {
    ball.y = PADDLE_Y - RADIUS;
    const angle = ((ball.x - paddle) / (PADDLE / 2 + RADIUS)) * 1.05;
    const speed = Math.min(210, 120 + (level - 1) * 14);
    velocity = { x: Math.sin(angle) * speed, y: -Math.cos(angle) * speed };
  }
  const hit = bricks.find(brick => ball.x + RADIUS >= brick.x && ball.x - RADIUS <= brick.x + 32 && ball.y + RADIUS >= brick.y && ball.y - RADIUS <= brick.y + 9);
  if (hit) {
    bricks = bricks.filter(brick => brick.id !== hit.id);
    if (previous.y + RADIUS <= hit.y || previous.y - RADIUS >= hit.y + 9) velocity.y *= -1;
    else velocity.x *= -1;
    score += (4 - Math.floor(hit.id / 8)) * 10;
    void recordBest();
    if (!bricks.length) {
      void recordBest();
      level++; bricks = newBricks(); status = "ready";
      ball = { x: paddle, y: PADDLE_Y - 5 };
      haptics.notification("success");
    }
  }
  if (ball.y > H + RADIUS) {
    lives--;
    void recordBest();
    status = lives ? "ready" : "over";
    ball = { x: paddle, y: PADDLE_Y - 5 };
    haptics.impact("light");
  }
}
function animate(time: number) {
  if (status !== "playing" || !visible || held) { frame = 0; return; }
  const elapsed = lastTime ? Math.min((time - lastTime) / 1000, 0.04) : 0;
  lastTime = time;
  // Small substeps keep collisions stable on slower devices.
  const steps = Math.max(1, Math.ceil(elapsed / 0.008));
  for (let i = 0; i < steps && status === "playing"; i++) step(elapsed / steps);
  frame = requestAnimationFrame(animate);
}
$effect(() => {
  if ((held || !visible) && status === "playing") status = "paused";
  if (status === "playing" && visible && !held && !frame) {
    lastTime = 0;
    frame = requestAnimationFrame(animate);
  }
});
function position(event: PointerEvent) {
  const box = arena.getBoundingClientRect();
  // xMidYMid meet letterboxes the 300×152 playfield without stretching it.
  const scale = Math.min(box.width / W, box.height / H);
  return (event.clientX - box.left - (box.width - W * scale) / 2) / scale;
}
function down(event: PointerEvent) {
  if (held || event.button !== 0) return;
  dragPointer = event.pointerId; dragStart = { x: event.clientX, y: event.clientY }; moved = false;
  arena.setPointerCapture(event.pointerId);
  movePaddle(position(event));
}
function drag(event: PointerEvent) {
  if (event.pointerId !== dragPointer) return;
  if (Math.hypot(event.clientX - dragStart.x, event.clientY - dragStart.y) > 5) moved = true;
  movePaddle(position(event));
}
function up(event: PointerEvent) {
  if (event.pointerId !== dragPointer) return;
  dragPointer = null;
  if (event.type !== "pointercancel" && !moved && status !== "playing") launch();
}
onMount(() => {
  visible = document.visibilityState === "visible";
  onattach(intent);
  return () => { cancelAnimationFrame(frame); onattach(null); };
});
</script>

<svelte:document onvisibilitychange={() => { visible = document.visibilityState === "visible"; }} />
<section class="pod-screen pod-breakout" aria-label="Brick Breaker">
  <TitleBar title="Brick Breaker" {playing} {paused} {held} />
  <div class="pod-game-score"><span>{String(score).padStart(4, "0")}</span><span>BEST {Math.max(best, score)}</span><span aria-label={`${lives} lives`}>{"●".repeat(lives)}{"○".repeat(3 - lives)}</span></div>
  <div class="pod-game-field">
    <svg bind:this={arena} class="pod-game-arena" viewBox={`0 0 ${W} ${H}`} role="img" aria-label="Brick field. Spin the wheel or drag to move the paddle." onpointerdown={down} onpointermove={drag} onpointerup={up} onpointercancel={up}>
      <rect width={W} height={H} fill="#131a24" />
      {#each bricks as brick (brick.id)}
        <rect x={brick.x} y={brick.y} width="32" height="9" rx="1.5" fill={["#ff788c", "#ffc775", "#b6df9a", "#8bc8fa"][Math.floor(brick.id / 8)]} />
        <path d={`M${brick.x + 2} ${brick.y + 2}h28`} stroke="#ffffff" stroke-opacity=".3" />
      {/each}
      <rect x={paddle - PADDLE / 2} y={PADDLE_Y} width={PADDLE} height="5" rx="2.5" fill="#e9eef5" />
      <circle cx={ball.x} cy={ball.y} r={RADIUS} fill="#ffffff" />
    </svg>
    {#if status !== "playing"}
      <div class="pod-game-prompt">
        <strong>{status === "over" ? "Nice run." : status === "paused" ? "Paused" : level > 1 ? `Level ${level}` : "Break a little."}</strong>
        <Button.Root disabled={held} onclick={launch}>{status === "over" ? "Play again" : status === "paused" ? "Resume" : "Launch ball"}</Button.Root>
      </div>
    {/if}
  </div>
  <div class="pod-game-hint" role="status">{status === "over" ? `${score} points · MENU to return` : "Spin or drag · ● launch / pause · MENU back"}</div>
</section>
