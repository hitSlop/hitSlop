<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import { backOut, cubicIn } from "svelte/easing";
  import FocusTimer from "../FocusTimer.svelte";
  import Invoice from "../Invoice.svelte";
  import RandomPicker from "../RandomPicker.svelte";
  import QuickChecklist from "./QuickChecklist.svelte";
  import DesktopPet from "./DesktopPet.svelte";

  type Kind = "window" | "shape" | "note" | "video";
  type Win = {
    id: string; title: string; emoji: string; kind: Kind;
    width: number; height: number; zoom?: number;
    fx: number; fy: number; tilt: number; open: boolean;
  };
  const layout: Win[] = [
    { id: "video", title: "hitSlop in action.mp4", emoji: "🎬", kind: "video", width: 460, height: 296, fx: 0, fy: .04, tilt: -1.5, open: true },
    { id: "picker", title: "Pick One.slop", emoji: "🔮", kind: "shape", width: 330, height: 208, zoom: .55, fx: .04, fy: 1, tilt: 1.5, open: true },
    { id: "checklist", title: "Today.slop", emoji: "☑️", kind: "window", width: 300, height: 392, fx: .39, fy: .1, tilt: 2, open: true },
    { id: "note", title: "Sticky note", emoji: "📝", kind: "note", width: 200, height: 176, fx: .325, fy: 1, tilt: -7, open: true },
    { id: "timer", title: "Pomodoro.slop", emoji: "🍅", kind: "shape", width: 250, height: 250, zoom: .72, fx: .64, fy: .78, tilt: 0, open: true },
    { id: "invoice", title: "Invoice.slop", emoji: "💸", kind: "window", width: 344, height: 392, zoom: .555, fx: 1, fy: .02, tilt: -2, open: true },
  ];
  // Medium desktops get a roomier arrangement; the invoice and picker wait in the dock.
  const medium: Record<string, Partial<Win>> = {
    video: { fx: 0, fy: .02 },
    checklist: { fx: 1, fy: .05 },
    timer: { fx: .3, fy: 1 },
    note: { fx: .75, fy: 1, tilt: -6 },
    invoice: { open: false, fx: .5, fy: .15 },
    picker: { open: false, fx: .45, fy: .55 },
  };
  let roomy = true;
  const clone = () => layout.map((win) => ({ ...win, ...(roomy ? {} : medium[win.id]) }));

  let windows = $state<Win[]>(clone());
  let order = $state<string[]>(layout.map((win) => win.id));
  let dragging = $state<string | null>(null);
  let dragTilt = $state(0);
  // Unknown until mounted, so server markup shows the desktop layout and small screens rely on CSS.
  let canDrag = $state<boolean | undefined>(undefined);
  let clock = $state("");
  let surface: HTMLDivElement;
  let drag: { id: string; x: number; y: number; left: number; top: number; spanX: number; spanY: number; lastX: number; lastT: number } | null = null;

  const PAD = 16;
  const DOCK = 88;
  const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
  const z = (id: string) => order.indexOf(id) + 1;

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
    order = layout.map((win) => win.id);
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
  function playVideo(): void {
    window.dispatchEvent(new CustomEvent("hitslop:play-video"));
  }

  function pointerDown(event: PointerEvent, win: Win): void {
    front(win.id);
    if (!canDrag || event.button !== 0) return;
    const target = event.target as HTMLElement;
    // Controls inside the title bar (the traffic lights) click; they never start a drag.
    if (target.closest("button")) return;
    const grip = target.closest("[data-grip]");
    // Titled windows move by their title bar; shaped slops move from anywhere that isn't a control.
    if (win.kind === "window" || win.kind === "video" ? !grip : target.closest("button, input, select, textarea, a, label, output")) return;
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
    const sync = () => { canDrag = query.matches; };
    sync();
    if (surface.clientWidth < 1100) {
      roomy = false;
      windows = clone();
    }
    query.addEventListener("change", sync);
    const format = new Intl.DateTimeFormat(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" });
    const tick = () => { clock = format.format(new Date()); };
    tick();
    const timer = setInterval(tick, 15_000);
    return () => { query.removeEventListener("change", sync); clearInterval(timer); };
  });
</script>

<div class="desktop" data-drag={canDrag === undefined ? undefined : String(canDrag)}>
  <div class="menubar" aria-hidden="true">
    <span class="menubar-app"><img src="/assets/appicon-128.webp" width="16" height="16" alt="" /><b>hitSlop</b></span>
    <span class="menubar-menus">File&nbsp;&nbsp;Edit&nbsp;&nbsp;Window&nbsp;&nbsp;Help</span>
    <span class="menubar-right"><span>◐</span><span>✦</span><span class="clock">{clock}</span></span>
  </div>

  <div class="surface" bind:this={surface}>
    {#each windows as win (win.id)}
      {#if win.open}
        <section
          class="win"
          data-win={win.id}
          data-kind={win.kind}
          data-dragging={dragging === win.id}
          aria-label={win.title}
          style:--w={`${win.width}px`}
          style:--h={`${win.height}px`}
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
          {#if win.kind === "window" || win.kind === "video"}
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

          <div class="content" style:zoom={win.zoom}>
            {#if win.id === "video"}
              <button type="button" class="poster" onclick={playVideo} aria-label="Watch hitSlop in action, 30 seconds">
                <picture>
                  <source type="image/avif" srcset="/assets/desktop-hero-poster-960.avif 960w, /assets/desktop-hero-poster.avif 1920w" sizes="460px" />
                  <source type="image/webp" srcset="/assets/desktop-hero-poster-960.webp 960w, /assets/desktop-hero-poster.webp 1920w" sizes="460px" />
                  <img src="/assets/desktop-hero-poster-960.jpg" width="960" height="540" alt="A Mac desktop full of open slops: a music player, a desktop pet, a focus timer, flashcards, a doodle board, a koi pond, Wordle and school planners" fetchpriority="high" />
                </picture>
                <span class="play"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l11-6.5Z" /></svg></span>
                <span class="play-label">Watch it in action <small>0:30</small></span>
              </button>
            {:else if win.id === "checklist"}
              <QuickChecklist />
            {:else if win.id === "timer"}
              <FocusTimer />
            {:else if win.id === "invoice"}
              <Invoice />
            {:else if win.id === "picker"}
              <RandomPicker />
            {:else if win.id === "note"}
              <div class="note"><p>Drag us around!</p><p>Every window here is a tiny app. <span>♥</span></p></div>
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
        <span aria-hidden="true">{win.emoji}</span><span class="tip">{win.title}</span>
      </button>
    {/each}
    <span class="dock-divider dock-tools" aria-hidden="true"></span>
    <button type="button" class="dock-app dock-tools" onclick={tidy} aria-label="Tidy up the desktop"><span aria-hidden="true">🧹</span><span class="tip">Tidy up</span></button>
    <button type="button" class="dock-app dock-tools" onclick={shuffle} aria-label="Shuffle the windows"><span aria-hidden="true">🎲</span><span class="tip">Shuffle</span></button>
  </nav>
</div>

<style>
  .desktop { --pad: 16px; --dock: 88px; position: relative; overflow: hidden; border: 2px solid #ffffffdd; border-radius: 30px; background: radial-gradient(ellipse at 18% 20%, #ffe3f4 0, transparent 42%), radial-gradient(ellipse at 85% 18%, #ddd3ff 0, transparent 48%), radial-gradient(ellipse at 70% 95%, #cff5ee 0, transparent 46%), linear-gradient(160deg, #f6f0ff, #eef4ff 60%, #fff4ea); box-shadow: 0 30px 70px #414d8230, 0 8px 0 #ffffff99; }
  .menubar { height: 34px; padding: 0 16px; display: flex; align-items: center; gap: 18px; color: #10132c; background: #ffffff8c; backdrop-filter: blur(12px); font-size: 13px; font-weight: 600; }
  .menubar-app { display: inline-flex; align-items: center; gap: 7px; }
  .menubar-app img { border-radius: 4px; }
  .menubar-menus { color: #10132cb3; }
  .menubar-right { margin-left: auto; display: inline-flex; gap: 14px; font-variant-numeric: tabular-nums; }
  .surface { position: relative; height: clamp(600px, 50vw, 700px); }
  .win { position: absolute; left: calc(var(--pad) + (100% - var(--pad) * 2 - var(--w)) * var(--fx)); top: calc(var(--pad) + (100% - var(--pad) - var(--dock) - var(--h)) * var(--fy)); width: var(--w); rotate: var(--tilt); transition: left 650ms var(--spring), top 650ms var(--spring), rotate 450ms var(--spring), box-shadow 200ms, scale 200ms var(--spring); touch-action: none; }
  .win[data-dragging="true"] { transition: rotate 120ms linear, box-shadow 200ms, scale 200ms var(--spring); scale: 1.03; cursor: grabbing; }
  .win[data-kind="window"], .win[data-kind="video"] { overflow: hidden; border: 1px solid #10132c1f; border-radius: 16px; background: #fff; box-shadow: 0 22px 44px #2a1b5c2e, 0 2px 6px #2a1b5c14; }
  .win[data-kind="window"][data-dragging="true"], .win[data-kind="video"][data-dragging="true"] { box-shadow: 0 40px 70px #2a1b5c40; }
  .bar { height: 36px; padding: 0 12px; display: flex; align-items: center; gap: 10px; border-bottom: 1px solid #10132c14; background: #f7f5fb; cursor: grab; user-select: none; }
  .bar .title { flex: 1; overflow: hidden; color: #10132c; font-size: 13px; font-weight: 800; text-align: center; text-overflow: ellipsis; white-space: nowrap; padding-right: 54px; }
  .lights { display: flex; gap: 7px; }
  .light { width: 13px; height: 13px; padding: 0; border: 0; border-radius: 50%; cursor: pointer; }
  .close { background: #ff5f57; } .min { background: #febc2e; } .zoom { background: #28c840; }
  .light:hover { filter: brightness(.9); scale: 1.15; }
  .grip-hint { position: absolute; z-index: 2; left: 50%; top: -30px; padding: 4px 10px; border-radius: 999px; color: #fff; background: #10132ccc; font-size: 12px; font-weight: 800; white-space: nowrap; opacity: 0; transform: translate(-50%, 6px); transition: opacity 160ms, transform 200ms var(--spring); cursor: grab; }
  .win:hover .grip-hint, .grip-hint:focus-visible { opacity: 1; transform: translate(-50%, 0); }
  .win[data-kind="shape"] { cursor: grab; filter: drop-shadow(0 18px 24px #2a1b5c30); }
  .content { display: grid; place-items: center; }
  .win[data-kind="window"] .content { display: block; }
  .poster { position: relative; display: block; width: 100%; padding: 0; border: 0; background: #10132c; cursor: pointer; }
  .poster img { display: block; width: 100%; height: auto; aspect-ratio: 16 / 9; object-fit: cover; }
  .play { position: absolute; left: 50%; top: 50%; width: 72px; height: 72px; display: grid; place-items: center; border-radius: 50%; background: #fffffff2; box-shadow: 0 12px 34px #2a1b6a66, 0 0 0 8px #ffffff55; transform: translate(-50%, -50%); transition: transform 240ms var(--spring); }
  .play svg { width: 30px; margin-left: 5px; fill: #6c3fd6; }
  .poster:hover .play, .poster:focus-visible .play { transform: translate(-50%, -50%) scale(1.12); }
  .play-label { position: absolute; left: 12px; bottom: 12px; padding: 6px 12px; border-radius: 999px; color: #10132c; background: #fffffff0; font-size: 14px; font-weight: 800; }
  .play-label small { margin-left: 4px; color: #6b6890; font-size: 13px; font-weight: 600; }
  .note { width: 100%; min-height: var(--h); padding: 22px 20px; color: #10132c; background: #fff19a; border-radius: 3px 6px 4px 2px; box-shadow: 0 14px 22px #6954222e; font-family: "HitSlop Handwriting", cursive; font-size: 21px; line-height: 1.15; cursor: grab; }
  .note p { margin: 0 0 10px; }
  .note span { color: #f443a1; }
  .dock { position: absolute; z-index: 60; left: 50%; bottom: 12px; padding: 8px 10px; display: flex; align-items: flex-end; gap: 6px; border: 1px solid #ffffffcc; border-radius: 24px; background: #ffffff9e; box-shadow: 0 14px 34px #2a1b5c26; backdrop-filter: blur(16px); transform: translateX(-50%); }
  .dock-app { position: relative; width: 52px; height: 52px; display: grid; place-items: center; padding: 0; border: 1px solid #10132c14; border-radius: 15px; background: #fff; font-size: 26px; cursor: pointer; transition: transform 260ms var(--spring); }
  .dock-app[data-open="true"]::after { position: absolute; bottom: -7px; width: 4px; height: 4px; border-radius: 50%; background: #10132c; content: ""; }
  .dock-app:hover, .dock-app:focus-visible { transform: translateY(-10px) scale(1.18); }
  .dock-app:active { transform: translateY(-4px) scale(1.05); }
  .tip { position: absolute; bottom: calc(100% + 10px); left: 50%; padding: 4px 9px; border-radius: 8px; color: #fff; background: #10132ce6; font-size: 12px; font-weight: 700; white-space: nowrap; opacity: 0; transform: translateX(-50%); pointer-events: none; transition: opacity 120ms; }
  .dock-app:hover .tip, .dock-app:focus-visible .tip { opacity: 1; }
  .dock-divider { width: 1px; height: 40px; margin: 0 4px 6px; background: #10132c26; }

  /* Touch and small screens: a swipeable strip of the same little apps, no dragging. */
  .desktop[data-drag="false"] .dock-tools { display: none; }
  .desktop[data-drag="false"] .surface { height: auto; padding: 18px 16px 104px; display: flex; align-items: center; gap: 16px; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; }
  .desktop[data-drag="false"] .win { position: relative; left: auto; top: auto; flex: none; rotate: 0deg; scroll-snap-align: center; }
  .desktop[data-drag="false"] .grip-hint { display: none; }
  .desktop[data-drag="false"] .bar { cursor: default; }
  @media (max-width: 779px) {
    .dock-tools { display: none; }
    .surface { height: auto !important; padding: 18px 16px 104px; display: flex; align-items: center; gap: 16px; overflow-x: auto; scroll-snap-type: x mandatory; scrollbar-width: none; }
    .win { position: relative !important; left: auto !important; top: auto !important; flex: none; rotate: 0deg !important; scroll-snap-align: center; max-width: calc(100vw - 64px); }
    .win[data-kind="video"] { width: min(var(--w), calc(100vw - 64px)); }
    .grip-hint, .menubar-menus { display: none; }
    .dock-app { width: 44px; height: 44px; font-size: 22px; }
  }
  @media (prefers-reduced-motion: reduce) { .win, .dock-app { transition: none; } }
</style>
