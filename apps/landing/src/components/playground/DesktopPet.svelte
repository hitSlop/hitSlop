<script lang="ts">
  import { onMount } from "svelte";

  // An original pixel blob in the app icon's colors. It strolls, pauses, hops and
  // blinks; clicking it says hi. It sleeps offscreen and never runs under reduced motion.
  const pixels = [
    "....aaaa....",
    "..aabbbbaa..",
    ".abbbbbbbba.",
    ".abbwkbwkba.",
    "abbbwkbwkbba",
    "abbbbbbbbbba",
    "abbbbccbbbba",
    "abbbbbbbbbba",
    ".abbbbbbbba.",
    "..aa.aa.aa..",
  ];
  const colors: Record<string, string> = { a: "#3a1f8f", b: "#8f5cf7", w: "#ffffff", k: "#10132c", c: "#ff7cc0" };
  const cells = pixels.flatMap((row, y) => [...row].flatMap((c, x) => (c === "." ? [] : [{ x, y, fill: colors[c]! }])));

  let root: HTMLButtonElement;
  let x = $state(40);
  let facing = $state(1);
  let hopping = $state(false);
  let blink = $state(false);
  let hearts = $state<number[]>([]);
  let moving = $state(true);
  let heartId = 0;

  function greet(): void {
    hopping = true;
    hearts = [...hearts, heartId++];
    setTimeout(() => { hopping = false; }, 500);
    setTimeout(() => { hearts = hearts.slice(1); }, 1400);
  }

  onMount(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let frame = 0, last = 0, visible = true, pauseUntil = 0;
    const loop = (time: number) => {
      const dt = last ? Math.min(time - last, 50) : 0;
      last = time;
      const width = root.parentElement?.clientWidth ?? 600;
      if (time > pauseUntil) {
        moving = true;
        x += facing * dt * 0.035;
        if (x < 8) { x = 8; facing = 1; }
        if (x > width - 64) { x = width - 64; facing = -1; }
        if (Math.random() < 0.002) { pauseUntil = time + 1400 + Math.random() * 1800; moving = false; if (Math.random() < 0.5) facing *= -1; }
      }
      if (Math.random() < 0.004) { blink = true; setTimeout(() => { blink = false; }, 140); }
      frame = requestAnimationFrame(loop);
    };
    const start = () => { cancelAnimationFrame(frame); last = 0; if (visible && !document.hidden) frame = requestAnimationFrame(loop); };
    const observer = new IntersectionObserver(([entry]) => { visible = !!entry?.isIntersecting; start(); });
    observer.observe(root);
    document.addEventListener("visibilitychange", start);
    start();
    return () => { cancelAnimationFrame(frame); observer.disconnect(); document.removeEventListener("visibilitychange", start); };
  });
</script>

<button class="pet" bind:this={root} type="button" aria-label="Say hi to Blob, the desktop pet" onclick={greet} style:--x={`${x}px`} data-hop={hopping} data-moving={moving}>
  <svg viewBox="0 0 12 10" style:transform={`scaleX(${facing})`} shape-rendering="crispEdges" aria-hidden="true">
    {#each cells as cell}<rect x={cell.x} y={cell.y} width="1.02" height="1.02" fill={blink && cell.fill === colors.k ? colors.b : cell.fill} />{/each}
  </svg>
  {#each hearts as id (id)}<span class="heart" aria-hidden="true">♥</span>{/each}
  <span class="hi" aria-hidden="true">hi!</span>
</button>

<style>
  .pet { position: absolute; z-index: 50; left: 0; bottom: 84px; width: 52px; height: 44px; padding: 0; border: 0; background: none; transform: translateX(var(--x)); cursor: pointer; }
  svg { width: 100%; height: 100%; display: block; filter: drop-shadow(0 4px 0 #10132c22); }
  .pet[data-moving="true"] svg { animation: waddle 420ms steps(2) infinite; }
  .pet[data-hop="true"] svg { animation: hop 480ms cubic-bezier(.34, 1.56, .64, 1); }
  .heart { position: absolute; left: 50%; top: -6px; color: #f443a1; font-size: 20px; animation: heart 1300ms ease-out forwards; pointer-events: none; }
  .hi { position: absolute; left: 40px; top: -26px; padding: 3px 8px; border: 2px solid #10132c; border-radius: 12px 12px 12px 3px; background: #fff; color: #10132c; font-family: "HitSlop Handwriting", cursive; font-size: 15px; line-height: 1; opacity: 0; transform: scale(.6); transition: opacity 160ms, transform 240ms cubic-bezier(.34, 1.56, .64, 1); pointer-events: none; }
  .pet:hover .hi, .pet:focus-visible .hi, .pet[data-hop="true"] .hi { opacity: 1; transform: none; }
  @keyframes waddle { 50% { translate: 0 -2px; rotate: 3deg; } }
  @keyframes hop { 40% { translate: 0 -18px; scale: .95 1.08; } 80% { translate: 0 0; scale: 1.08 .92; } }
  @keyframes heart { to { translate: -50% -56px; opacity: 0; scale: 1.5; } from { translate: -50% 0; } }
  @media (prefers-reduced-motion: reduce) { .pet { display: none; } }
</style>
