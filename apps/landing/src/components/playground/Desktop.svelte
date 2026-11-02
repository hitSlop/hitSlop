<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import { backOut, cubicIn } from "svelte/easing";
  import FocusTimer from "../FocusTimer.svelte";
  import LittleDays from "./LittleDays.svelte";
  import PocketPod from "./PocketPod.svelte";
  import DesktopPet from "./DesktopPet.svelte";

  type Kind = "window" | "shape";
  type Win = {
    id: string; title: string; emoji: string; icon?: string; kind: Kind;
    width: number; height: number; zoom?: number;
    fx: number; fy: number; tilt: number; open: boolean;
  };
  // Sizes are design pixels; `k` scales them to the room the desktop has.
  const layout: Win[] = [
    { id: "timer", title: "Focus Timer.pop", emoji: "🍅", kind: "shape", width: 260, height: 260, zoom: .667, fx: .12, fy: .68, tilt: -5, open: true },
    { id: "journal", title: "Little Days.pop", emoji: "🍼", icon: "/assets/templates/baby-journal/icon.png", kind: "window", width: 540, height: 398, zoom: .6, fx: .43, fy: .04, tilt: 1, open: true },
    { id: "pod", title: "Pocket Pod.pop", emoji: "🎵", icon: "/assets/templates/pocket-pod/icon.png", kind: "shape", width: 223, height: 374, zoom: .62, fx: .83, fy: .12, tilt: 3, open: true },
  ];
  const clone = () => layout.map((win) => ({ ...win }));
  const stack = () => ["journal", "pod", "timer"];

  let windows = $state<Win[]>(clone());
  let order = $state<string[]>(stack());
  let dragging = $state<string | null>(null);
  let dragTilt = $state(0);
  // Unknown until mounted, so server markup shows the desktop layout and small screens rely on CSS.
  let canDrag = $state<boolean | undefined>(undefined);
  let k = $state(1);
  let clock = $state("");
  let surface: HTMLDivElement;
  let drag: { id: string; x: number; y: number; left: number; top: number; spanX: number; spanY: number; lastX: number; lastT: number } | null = null;

  const PAD = 16;
  const DOCK = 72;
  const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
  const z = (id: string) => order.indexOf(id) + 1;
  const size = (win: Win) => ({ w: Math.round(win.width * k), h: Math.round(win.height * k), zoom: win.zoom ? win.zoom * k : undefined });

  function front(id: string): void {
    if (order.at(-1) !== id) order = [...order.filter((other) => other !== id), id];
  }
  function open(win: Win): void {
    win.open = true;
    front(win.id);
    // In the swipeable strip, the dock scrolls to the app instead.
    if (!canDrag) requestAnimationFrame(() => surface.querySelector(`[data-win="${win.id}"]`)?.scrollIntoView({ behavior: "smooth", inline: "center", block: "nearest" }));
  }
  function tidy(): void {
    windows = clone();
    order = stack();
  }
  function shuffle(): void {
    for (const win of windows) {
      win.open = true;
      win.fx = Math.random();
      win.fy = Math.random();
      win.tilt = Math.round((Math.random() * 10 - 5) * 10) / 10;
    }
    order = [...order].sort(() => Math.random() - .5);
  }

  function pointerDown(event: PointerEvent, win: Win): void {
    front(win.id);
    if (!canDrag || event.button !== 0) return;
    const target = event.target as HTMLElement;
    // Controls inside the title bar (the traffic lights) click; they never start a drag.
    if (target.closest("button")) return;
    const grip = target.closest("[data-grip]");
    // Titled windows move by their title bar; shaped pop move from anywhere that isn't a control.
    if (win.kind === "window" ? !grip : target.closest("button, input, select, textarea, a, label, output")) return;
    const element = event.currentTarget as HTMLElement;
    const box = surface.getBoundingClientRect();
    const rect = element.getBoundingClientRect();
    drag = {
      id: win.id, x: event.clientX, y: event.clientY,
      left: rect.left - box.left - PAD, top: rect.top - box.top - PAD,
      spanX: box.width - PAD * 2 - element.offsetWidth, spanY: box.height - PAD - DOCK - element.offsetHeight,
      lastX: event.clientX, lastT: event.timeStamp,
    };
    dragging = win.id;
    element.setPointerCapture(event.pointerId);
    event.preventDefault();
  }
  function pointerMove(event: PointerEvent, win: Win): void {
    if (!drag || drag.id !== win.id) return;
    const left = clamp(drag.left + event.clientX - drag.x, 0, Math.max(0, drag.spanX));
    const top = clamp(drag.top + event.clientY - drag.y, 0, Math.max(0, drag.spanY));
    win.fx = drag.spanX > 0 ? left / drag.spanX : 0;
    win.fy = drag.spanY > 0 ? top / drag.spanY : 0;
    // Lean into the motion a little, like paper being slid across a desk.
    const velocity = (event.clientX - drag.lastX) / Math.max(1, event.timeStamp - drag.lastT);
    dragTilt = clamp(velocity * 7, -9, 9);
    drag.lastX = event.clientX;
    drag.lastT = event.timeStamp;
  }
  function pointerUp(): void {
    drag = null;
    dragging = null;
    dragTilt = 0;
  }
  function nudge(event: KeyboardEvent, win: Win): void {
    const step = event.shiftKey ? .15 : .04;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const move = moves[event.key];
    if (!move) return;
    event.preventDefault();
    front(win.id);
    win.fx = clamp(win.fx + move[0], 0, 1);
    win.fy = clamp(win.fy + move[1], 0, 1);
  }

  onMount(() => {
    const query = matchMedia("(min-width: 780px) and (pointer: fine)");
    const fit = () => {
      canDrag = query.matches;
      const width = surface.clientWidth;
      // Desktop: shrink gently with the room. Strip: the widest window must fit a phone screen.
      // Fit the room both ways: a short desktop shrinks the windows instead of clipping them.
      k = canDrag ? clamp(Math.min(width / 1380, (surface.clientHeight - PAD - DOCK) / 400), .55, 1) : clamp((width - 24) / 540, .5, 1);
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(surface);
    query.addEventListener("change", fit);
    const format = new Intl.DateTimeFormat(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" });
    const tick = () => { clock = format.format(new Date()); };
    tick();
    const timer = setInterval(tick, 15_000);
    return () => { observer.disconnect(); query.removeEventListener("change", fit); clearInterval(timer); };
  });
</script>

<div class="desktop" data-drag={canDrag === undefined ? undefined : String(canDrag)}>
  <div class="menubar" aria-hidden="true">
    <span class="menubar-app"><img src="/assets/brand/mark.webp" width="18" height="18" alt="" /><b>dotpop</b></span>
    <span class="menubar-menus">File&nbsp;&nbsp;Edit&nbsp;&nbsp;Window&nbsp;&nbsp;Help</span>
    <span class="menubar-right"><span>◐</span><span>✦</span><span class="clock">{clock}</span></span>
  </div>

  <div class="surface" bind:this={surface}>
    {#each windows as win (win.id)}
      {#if win.open}
        {@const dims = size(win)}
        <section
          class="win"
          data-win={win.id}
          data-kind={win.kind}
          data-dragging={dragging === win.id}
          aria-label={win.title}
          style:--w={`${dims.w}px`}
          style:--h={`${dims.h}px`}
          style:--fx={win.fx}
          style:--fy={win.fy}
          style:--tilt={`${win.tilt + (dragging === win.id ? dragTilt : 0)}deg`}
          style:z-index={z(win.id)}
          onpointerdown={(event) => pointerDown(event, win)}
          onpointermove={(event) => pointerMove(event, win)}
          onpointerup={pointerUp}
          onpointercancel={pointerUp}
          onfocusin={() => front(win.id)}
          in:scale={{ start: .4, duration: 420, easing: backOut }}
          out:scale={{ start: .4, duration: 200, easing: cubicIn }}
        >
          {#if win.kind === "window"}
            <div class="bar" data-grip role="button" tabindex="0" aria-label={`Move ${win.title}. Use the arrow keys.`} onkeydown={(event) => nudge(event, win)}>
              <span class="lights">
                <button type="button" class="light close" aria-label={`Close ${win.title}`} onclick={() => (win.open = false)}></button>
                <button type="button" class="light min" aria-label={`Hide ${win.title}`} onclick={() => (win.open = false)}></button>
                <button type="button" class="light zoom" aria-label={`Bring ${win.title} to front`} onclick={() => front(win.id)}></button>
              </span>
              <span class="title">{win.title}</span>
            </div>
          {:else}
            <span class="grip-hint" data-grip role="button" tabindex="0" aria-label={`Move ${win.title}. Use the arrow keys.`} onkeydown={(event) => nudge(event, win)}>{win.title}</span>
          {/if}

          <div class="content" style:zoom={dims.zoom}>
            {#if win.id === "journal"}
              <LittleDays />
            {:else if win.id === "timer"}
              <FocusTimer />
            {:else if win.id === "pod"}
              <PocketPod />
            {/if}
          </div>
        </section>
      {/if}
    {/each}
    <DesktopPet />
  </div>

  <nav class="dock" aria-label="Playground dock">
    {#each windows as win (win.id)}
      <button type="button" class="dock-app" data-open={win.open} onclick={() => open(win)} aria-label={win.open ? `Bring ${win.title} to front` : `Open ${win.title}`}>
        {#if win.icon}<img src={win.icon} width="52" height="52" alt="" loading="lazy" />{:else}<span aria-hidden="true">{win.emoji}</span>{/if}<span class="tip">{win.title}</span>
      </button>
    {/each}
    <span class="dock-divider dock-tools" aria-hidden="true"></span>
    <button type="button" class="dock-app dock-tools" onclick={tidy} aria-label="Tidy up the desktop"><span aria-hidden="true">🧹</span><span class="tip">Tidy up</span></button>
    <button type="button" class="dock-app dock-tools" onclick={shuffle} aria-label="Shuffle the windows"><span aria-hidden="true">🎲</span><span class="tip">Shuffle</span></button>
  </nav>
</div>

<style>
  .desktop { --pad: 16px; --dock: 88px; position: relative; z-index: 1; overflow: hidden; border: 2px solid #ffffffdd; border-radius: 26px; background: radial-gradient(ellipse at 12% 100%, #ffe4ef 0, transparent 52%), radial-gradient(ellipse at 100% 80%, #dff3f8 0, transparent 48%), #fffaf1; box-shadow: 0 24px 50px #6b4a2a24, 0 6px 0 #ffffff99; }
  .menubar { height: 34px; padding: 0 16px; display: flex; align-items: center; gap: 18px; color: #14102a; background: #ffffff8c; backdrop-filter: blur(12px); font-size: 13px; font-weight: 600; }
  .menubar-app { display: inline-flex; align-items: center; gap: 7px; }
  .menubar-app img { border-radius: 4px; }
  .menubar-menus { color: #14102ab3; }
  .menubar-right { margin-left: auto; display: inline-flex; gap: 14px; font-variant-numeric: tabular-nums; }
  .surface { position: relative; height: clamp(330px, calc(100svh - 470px), 460px); }
  .win { position: absolute; left: calc(var(--pad) + (100% - var(--pad) * 2 - var(--w)) * var(--fx)); top: calc(var(--pad) + (100% - var(--pad) - var(--dock) - var(--h)) * var(--fy)); width: var(--w); rotate: var(--tilt); transition: left 650ms var(--spring), top 650ms var(--spring), rotate 450ms var(--spring), box-shadow 200ms, scale 200ms var(--spring); touch-action: none; }
  .win[data-dragging="true"] { transition: rotate 120ms linear, box-shadow 200ms, scale 200ms var(--spring); scale: 1.03; cursor: grabbing; }
  .win[data-kind="window"] { overflow: hidden; border: 1px solid #14102a1f; border-radius: 16px; background: #fff; box-shadow: 0 22px 44px #3a24142e, 0 2px 6px #3a241414; }
  .win[data-kind="window"][data-dragging="true"] { box-shadow: 0 40px 70px #3a241440; }
  .bar { height: 36px; padding: 0 12px; display: flex; align-items: center; gap: 10px; border-bottom: 1px solid #14102a14; background: #fbf6ea; cursor: grab; user-select: none; }
  .bar .title { flex: 1; overflow: hidden; color: #14102a; font-size: 13px; font-weight: 800; text-align: center; text-overflow: ellipsis; white-space: nowrap; padding-right: 54px; }
  .lights { display: flex; gap: 7px; }
  .light { width: 13px; height: 13px; padding: 0; border: 0; border-radius: 50%; cursor: pointer; }
  .close { background: #ff5f57; } .min { background: #febc2e; } .zoom { background: #28c840; }
  .light:hover { filter: brightness(.9); scale: 1.15; }
  .grip-hint { position: absolute; z-index: 2; left: 50%; top: -30px; padding: 4px 10px; border-radius: 999px; color: #fff; background: #14102acc; font-size: 12px; font-weight: 800; white-space: nowrap; opacity: 0; transform: translate(-50%, 6px); transition: opacity 160ms, transform 200ms var(--spring); cursor: grab; }
  .win:hover .grip-hint, .grip-hint:focus-visible { opacity: 1; transform: translate(-50%, 0); }
  .win[data-kind="shape"] { cursor: grab; filter: drop-shadow(0 18px 24px #3a241430); }
  .content { display: grid; place-items: center; }
  .win[data-kind="window"] .content { display: block; }
  .dock { position: absolute; z-index: 60; left: 50%; bottom: 12px; padding: 8px 10px; display: flex; align-items: flex-end; gap: 6px; border: 1px solid #ffffffcc; border-radius: 24px; background: #ffffff9e; box-shadow: 0 14px 34px #3a241426; backdrop-filter: blur(16px); transform: translateX(-50%); }
  .dock-app { position: relative; width: 52px; height: 52px; display: grid; place-items: center; padding: 0; overflow: visible; border: 1px solid #14102a14; border-radius: 15px; background: #fffdf8; font-size: 26px; cursor: pointer; transition: transform 260ms var(--spring); }
  .dock-app[data-open="true"]::after { position: absolute; bottom: -7px; width: 4px; height: 4px; border-radius: 50%; background: #14102a; content: ""; }
  .dock-app:hover, .dock-app:focus-visible { transform: translateY(-10px) scale(1.18); }
  .dock-app:active { transform: translateY(-4px) scale(1.05); }
  .tip { position: absolute; bottom: calc(100% + 10px); left: 50%; padding: 4px 9px; border-radius: 8px; color: #fff; background: #14102ae6; font-size: 12px; font-weight: 700; white-space: nowrap; opacity: 0; transform: translateX(-50%); pointer-events: none; transition: opacity 120ms; }
  .dock-app:hover .tip, .dock-app:focus-visible .tip { opacity: 1; }
  .dock-app img { width: 100%; height: 100%; border-radius: inherit; object-fit: cover; }
  .dock-divider { width: 1px; height: 40px; margin: 0 4px 6px; background: #14102a26; }

  /* Touch and small screens: a swipeable strip of the same little apps, no dragging. */
  .desktop[data-drag="false"] .dock-tools { display: none; }
  .desktop[data-drag="false"] .surface { height: auto; padding: 18px 16px 104px; display: flex; align-items: center; gap: 16px; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; }
  .desktop[data-drag="false"] .win { position: relative; left: auto; top: auto; flex: none; rotate: 0deg; scroll-snap-align: center; }
  .desktop[data-drag="false"] .grip-hint { display: none; }
  .desktop[data-drag="false"] .bar { cursor: default; }
  @media (max-width: 779px) {
    .desktop { border-radius: 20px; box-shadow: 0 14px 30px #6b4a2a20, 0 4px 0 #ffffff99; }
    .menubar { padding-inline: 12px; gap: 8px; font-size: 12px; }
    .menubar-right { gap: 8px; }
    .dock-tools { display: none; }
    .surface, .desktop[data-drag="false"] .surface { height: auto !important; padding: 12px 12px 88px; display: flex; align-items: flex-start; gap: 16px; overflow-x: auto; scroll-padding-inline: 12px; scroll-snap-type: x mandatory; scrollbar-width: none; }
    .win { position: relative !important; left: auto !important; top: auto !important; flex: none; rotate: 0deg !important; scroll-snap-align: start; max-width: 100%; touch-action: auto; }
      .bar { padding-inline: 8px; gap: 6px; }
    .bar .title { padding-right: 0; font-size: 11px; }
    .grip-hint, .menubar-menus { display: none; }
    .dock { max-width: calc(100% - 16px); padding: 8px 6px 12px; gap: 4px; overflow-x: auto; border-radius: 18px; }
    .dock-app { width: 44px; height: 44px; flex: 0 0 44px; font-size: 22px; }
    .dock-app:hover, .dock-app:focus-visible { transform: none; }
    .tip { display: none; }
  }
  @media (prefers-reduced-motion: reduce) { .win, .dock-app { transition: none; } }
</style>
