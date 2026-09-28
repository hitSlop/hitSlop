<script lang="ts">
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { onDestroy, onMount, tick } from "svelte";
  import { capture } from "@hitslop/document/capture";
  import { AlertDialog, Collapsible, Popover, RadioGroup, ToggleGroup } from "bits-ui";
  import Pin from "@lucide/svelte/icons/pin";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import Maximize from "@lucide/svelte/icons/maximize";
  import Eraser from "@lucide/svelte/icons/eraser";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import schema, { boardShapes } from "./schema";
  import { boards, markers, widths, StrokeSamples, strokePath, sweep, defaultBrush, fitWindowSize, type Brush, type Point } from "./drawing";
  import BrushControls from "./BrushControls.svelte";

  const doc = useDocument(schema);
  const board = $derived(boards[doc.current.boardShape]);
  let marker = $state<(typeof markers)[number]>("charcoal");
  let brush = $state<Brush>({ ...defaultBrush });
  let tool = $state<"pen" | "eraser">("pen");
  let boardMenu = $state(false);
  let clearDialog = $state(false);
  let brushMenu = $state(false);
  let trayOpen = $state(false);
  let pinned = $state(false);
  let drawing = $state(false);
  let hovered = false;
  let openedByHover = false;
  let trayFocused = false;
  let dismissTimer: ReturnType<typeof setTimeout> | undefined;
  let trayRoot: HTMLDivElement;
  let toolsTab = $state<HTMLButtonElement | null>(null);
  let shell: HTMLElement;
  let stage: HTMLElement;
  let alive = true;
  let fitVersion = 0;
  let resizeQueue = Promise.resolve();
  let nativeResize = $state(false);
  let svg: SVGSVGElement;
  let active: { pointer: number; id?: string; samples: StrokeSamples; brush: Brush; pen: boolean; last: Point; erased: Set<string>; eraserPaths: { id: string; path: Path2D }[] } | undefined;
  let hitContext: CanvasRenderingContext2D | null = null;
  let frame = 0;
  let notice = $state("");
  $effect(() => { if (notice) { const timer = setTimeout(() => notice = "", 3500); return () => clearTimeout(timer); } });
  $effect(() => { if (!brushMenu && !clearDialog && !pinned) scheduleDismiss(); });

  function keepTray() { clearTimeout(dismissTimer); }
  function scheduleDismiss() {
    keepTray();
    dismissTimer = setTimeout(() => { if (!hovered && !trayFocused && !pinned && !brushMenu && !clearDialog) trayOpen = false; }, 300);
  }
  function enterTray(event: PointerEvent) {
    if (drawing || event.buttons || event.pointerType === "touch") return;
    openedByHover = !trayOpen;
    hovered = true; keepTray(); trayOpen = true;
  }
  function clickTools(event: MouseEvent) {
    if (openedByHover) { event.preventDefault(); trayOpen = true; keepTray(); }
    openedByHover = false;
  }
  function leaveTray() { hovered = false; scheduleDismiss(); }
  function focusTray() { trayFocused = true; keepTray(); }
  function blurTray() {
    queueMicrotask(() => { if (!alive) return; trayFocused = trayRoot?.contains(document.activeElement) ?? false; scheduleDismiss(); });
  }
  function closeTray() { if (pinned) return; trayOpen = false; if (trayRoot?.contains(document.activeElement)) toolsTab?.focus(); }
  function changeBrush(value: Brush) { finish(); brush = value; }
  function fitBoard() {
    finish();
    const version = ++fitVersion;
    resizeQueue = resizeQueue.then(async () => {
      await tick();
      const resize = (globalThis as typeof globalThis & { slop?: { window: { resize: (size: { width: number; height: number }) => Promise<{ width: number; height: number }> } } }).slop?.window.resize;
      if (!alive || version !== fitVersion || !resize || capture.isRenderer()) return;
      const chrome = { width: shell.clientWidth - stage.clientWidth, height: shell.clientHeight - stage.clientHeight };
      const desired = fitWindowSize(doc.current.boardShape, chrome, { width: screen.availWidth, height: screen.availHeight });
      // The host may further clamp to this window's display; CSS fits its actual viewport.
      await resize(desired);
    }).catch(() => { if (alive) notice = "Couldn't fit the window. Try Fit board again."; });
  }
  onMount(() => {
    let observer: ResizeObserver | undefined;
    // Slop mounts its child snippet inside a boundary; wait for its element bindings.
    void tick().then(() => {
      if (!alive || !shell) return;
      nativeResize = Boolean((globalThis as any).slop?.window?.resize) && !capture.isRenderer();
      if (nativeResize) fitBoard();
      let previous = { width: shell.clientWidth, height: shell.clientHeight };
      observer = new ResizeObserver(() => {
        const next = { width: shell.clientWidth, height: shell.clientHeight };
        if (next.width !== previous.width || next.height !== previous.height) finish();
        previous = next;
      });
      observer.observe(shell);
    });
    return () => observer?.disconnect();
  });

  function point(event: PointerEvent): Point | undefined {
    const matrix = svg?.getScreenCTM();
    if (!matrix) return;
    const p = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
    return [p.x, p.y, event.pointerType === "pen" ? event.pressure || 0.5 : 0.5];
  }
  function inside(p: Point) { return p[0] >= 0 && p[1] >= 0 && p[0] <= board.width && p[1] <= board.height; }
  function pathForGesture() { return active ? strokePath(active.samples.points, active.brush, active.pen) : ""; }
  function preview() {
    frame = 0;
    if (!active?.id || !doc.current.strokes.some(s => s.$id === active!.id)) return;
    doc.fields.strokes.item(active.id).geometry.preview(pathForGesture());
  }
  function finish() {
    cancelAnimationFrame(frame); frame = 0;
    if (!active) return;
    const gesture = active;
    if (gesture.id && doc.current.strokes.some(s => s.$id === gesture.id)) {
      doc.fields.strokes.item(gesture.id).geometry.set(pathForGesture());
    }
    active = undefined;
    drawing = false;
    if (svg?.hasPointerCapture(gesture.pointer)) svg.releasePointerCapture(gesture.pointer);
  }
  function erase(from: Point, to: Point) {
    if (!active || !hitContext) return;
    const paths = active.eraserPaths;
    const removed = new Set<string>();
    for (const p of sweep(from, to)) {
      if (!inside(p)) continue;
      // At each sample only the topmost visible stroke is touched.
      const hit = paths.find(stroke => hitContext!.isPointInPath(stroke.path, p[0], p[1]));
      if (hit && !active.erased.has(hit.id) && doc.current.strokes.some(stroke => stroke.$id === hit.id)) removed.add(hit.id);
    }
    if (removed.size) {
      doc.change(tx => { for (const id of removed) tx.fields.strokes.remove(id); }, { message: "Erase strokes" });
      for (const id of removed) active.erased.add(id);
    }
  }
  function start(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary || active) return;
    const p = point(event); if (!p || !inside(p)) return;
    event.preventDefault();
    drawing = true; hovered = false;
    if (!pinned) { trayOpen = false; (document.activeElement as HTMLElement | null)?.blur(); }
    const samples = new StrokeSamples(); samples.add(p);
    active = { pointer: event.pointerId, samples, brush: { ...brush }, pen: event.pointerType === "pen", last: p, erased: new Set(), eraserPaths: [] };
    if (tool === "pen") {
      const color = getComputedStyle(svg).getPropertyValue(`--slop-${marker}`).trim();
      active.id = doc.fields.strokes.insert({ geometry: pathForGesture(), color }).id;
    } else {
      hitContext ??= document.createElement("canvas").getContext("2d");
      // Freeze hit-test order for this gesture so pointer-up cannot erase a newly exposed stroke.
      active.eraserPaths = [...doc.current.strokes].reverse().map(stroke => ({ id: stroke.$id, path: new Path2D(stroke.geometry) }));
      erase(p, p);
    }
    svg.setPointerCapture(event.pointerId);
  }
  function move(event: PointerEvent) {
    if (!active || event.pointerId !== active.pointer) return;
    const p = point(event); if (!p) return;
    if (active.id) {
      for (const sample of event.getCoalescedEvents?.() ?? []) { const next = point(sample); if (next) active.samples.add(next); }
      active.samples.add(p);
      if (!frame) frame = requestAnimationFrame(preview);
    } else erase(active.last, p);
    active.last = p;
  }
  function end(event: PointerEvent) {
    if (!active || active.pointer !== event.pointerId) return;
    const p = point(event);
    if (p && event.type === "pointerup") {
      if (active.id) active.samples.add(p, true);
      else erase(active.last, p);
    }
    finish();
  }
  function chooseMarker(value: string) {
    if (!markers.includes(value as typeof marker)) return;
    finish(); marker = value as typeof marker; tool = "pen";
  }
  function chooseBoard(value: string) {
    if (!boardShapes.includes(value as typeof doc.current.boardShape)) return;
    finish(); doc.fields.boardShape.set(value as typeof doc.current.boardShape); boardMenu = false;
    fitBoard();
  }
  function clearBoard() {
    finish();
    doc.change(tx => { for (const stroke of doc.current.strokes) tx.fields.strokes.remove(stroke.$id); }, { message: "Clear board" });
    clearDialog = false;
    notice = "Fresh start.";
  }
  function shortcut(event: KeyboardEvent) {
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || event.target instanceof HTMLInputElement || (event.target as HTMLElement)?.isContentEditable || boardMenu || clearDialog || brushMenu) return;
    if (event.key === "Escape") { closeTray(); return; }
    if (event.key.toLowerCase() === "t" && !drawing) { event.preventDefault(); if (trayOpen) closeTray(); else { trayOpen = true; keepTray(); toolsTab?.focus(); } }
    if (event.key.toLowerCase() === "e") { finish(); tool = "eraser"; }
    if (event.key.toLowerCase() === "p") { finish(); tool = "pen"; }
  }
  onDestroy(() => { alive = false; fitVersion++; cancelAnimationFrame(frame); keepTray(); });
</script>

<svelte:window onblur={finish} onkeydown={shortcut} />

<Slop>
  <main bind:this={shell} class="doodle-shell" data-slop-selection="none" data-drawing={drawing}>
    <header class="doodle-header">
      <div class="doodle-wordmark"><svg viewBox="0 0 36 32" aria-hidden="true"><path d="M3 20Q10 0 15 12T25 11Q36 3 29 21T14 23Q5 16 3 28" /></svg><h1>Doodle Board</h1></div>
      <Popover.Root bind:open={boardMenu}>
        <Popover.Trigger class="doodle-board-control" onclick={finish} aria-label={`Board size: ${board.label}`}><span class="doodle-shape" data-shape={doc.current.boardShape}></span><span>{board.label}</span><ChevronDown size={15} /></Popover.Trigger>
        <Popover.Portal><Popover.Content class="doodle-popover" collisionPadding={12} sideOffset={9} align="end">
          <h2>Board size</h2>
          <RadioGroup.Root value={doc.current.boardShape} onValueChange={chooseBoard} aria-label="Board size" class="doodle-presets">
            {#each boardShapes as shape}<RadioGroup.Item value={shape} class="doodle-preset"><span class="doodle-shape" data-shape={shape}></span><span>{boards[shape].label}<small>{boards[shape].width} × {boards[shape].height}</small></span>{#if doc.current.boardShape === shape}<Check size={17} />{/if}</RadioGroup.Item>{/each}
          </RadioGroup.Root>
          <button class="doodle-fit" disabled={!nativeResize} onclick={() => { boardMenu = false; fitBoard(); }}><Maximize size={16} /> Fit board</button>
          {#if !nativeResize}<p>Window fitting is available in the Mac app.</p>{/if}
          <p>Anything outside the board stays tucked away until you make it bigger.</p>
        </Popover.Content></Popover.Portal>
      </Popover.Root>
    </header>

    <section bind:this={stage} class="doodle-stage" aria-label="Drawing area">
      <div class="doodle-board" style:aspect-ratio={`${board.width} / ${board.height}`} style:width={`min(100cqw, ${board.width / board.height * 100}cqh)`}>
        <svg bind:this={svg} class="doodle-canvas" class:doodle-erasing={tool === "eraser"} viewBox={`0 0 ${board.width} ${board.height}`} aria-label={`Whiteboard, ${board.label.toLowerCase()}. Draw with a pointer. P for pen, E for eraser.`} role="img" onpointerdown={start} onpointermove={move} onpointerup={end} onpointercancel={end} onlostpointercapture={end}>
          {#each doc.current.strokes as stroke (stroke.$id)}<path data-stroke={stroke.$id} d={stroke.geometry} fill={stroke.color} />{/each}
        </svg>
        {#if !doc.current.strokes.length}<div class="doodle-invitation" aria-hidden="true"><span class="doodle-invitation-star">✳</span><p>Make a little<br /><em>something.</em></p><span>No wrong lines here.</span></div>{/if}
      </div>
    </section>

    <footer class="doodle-dock">
    <Collapsible.Root bind:open={trayOpen} class="doodle-disclosure">
    <div bind:this={trayRoot} class="doodle-tray-root" role="group" aria-label="Drawing tools" onpointerenter={enterTray} onpointerleave={leaveTray} onfocusin={focusTray} onfocusout={blurTray}>
    <Collapsible.Content class="doodle-tray" id="doodle-tools">
      <div class="doodle-tray-heading"><span>Your little toolkit</span><button class="doodle-pin" aria-label="Pin tools" aria-pressed={pinned} onclick={() => { pinned = !pinned; if (pinned) { trayOpen = true; keepTray(); } else scheduleDismiss(); }}><Pin size={15} />{pinned ? "Pinned" : "Pin tools"}</button></div>
      <div class="doodle-tools">
        <ToggleGroup.Root type="single" value={tool === "pen" ? marker : ""} onValueChange={chooseMarker} class="doodle-markers" aria-label="Marker color">
          {#each markers as color}<ToggleGroup.Item value={color} class="doodle-marker" style={`--marker: var(--slop-${color})`} aria-label={`${color[0]!.toUpperCase() + color.slice(1)} marker`}><span class="doodle-marker-tip"></span><span class="doodle-marker-body"><i></i><span>DOODLE</span><b></b></span><span class="doodle-marker-dot"></span></ToggleGroup.Item>{/each}
        </ToggleGroup.Root>
        <div class="doodle-tool-divider"></div>
        <div class="doodle-widths"><span class="doodle-label">LINE WEIGHT</span><RadioGroup.Root value={String(brush.size)} onValueChange={(value) => changeBrush({ ...brush, size: Number(value) })} aria-label="Line weight"><div class="doodle-width-options">{#each widths as weight}<RadioGroup.Item value={String(weight.size)} aria-label={weight.label} class="doodle-width"><span style:width={`${weight.size / 1.4 + 3}px`} style:height={`${weight.size / 1.4 + 3}px`}></span></RadioGroup.Item>{/each}</div></RadioGroup.Root></div>
        <Popover.Root bind:open={brushMenu}>
          <Popover.Trigger class="doodle-brush-trigger" onclick={finish}><SlidersHorizontal size={18} /><span>Brush</span></Popover.Trigger>
          <Popover.Portal><Popover.Content class="doodle-popover doodle-brush-popover" collisionPadding={12} sticky="always" side="top" sideOffset={12} align="end" onEscapeKeydown={(event) => event.stopPropagation()}><BrushControls {brush} color={`var(--slop-${marker})`} onchange={changeBrush} /></Popover.Content></Popover.Portal>
        </Popover.Root>
        <button class="doodle-eraser" aria-label="Eraser" aria-pressed={tool === "eraser"} onclick={() => { finish(); tool = tool === "eraser" ? "pen" : "eraser"; }}><span><Eraser size={23} strokeWidth={1.8} /></span><small>Eraser</small></button>
        <AlertDialog.Root bind:open={clearDialog}>
          <AlertDialog.Trigger class="doodle-clear" disabled={!doc.current.strokes.length} onclick={finish}><Trash2 size={17} /><span>Clear board</span></AlertDialog.Trigger>
          <AlertDialog.Portal><AlertDialog.Overlay class="doodle-dialog-overlay" /><AlertDialog.Content class="doodle-dialog"><Sparkles size={26} /><AlertDialog.Title>Make room for something new?</AlertDialog.Title><AlertDialog.Description>This clears every stroke, including anything tucked outside the board. It can’t be undone.</AlertDialog.Description><div class="doodle-dialog-actions"><AlertDialog.Cancel>Keep drawing</AlertDialog.Cancel><AlertDialog.Action onclick={clearBoard}>Clear board</AlertDialog.Action></div></AlertDialog.Content></AlertDialog.Portal>
        </AlertDialog.Root>
      </div>
      <div class="doodle-tray-foot"><span>{tool === "eraser" ? "Touch a line to erase the whole stroke." : "A color, a line, a little possibility."}</span><span class="doodle-key-hint">P <i>pen</i> · E <i>eraser</i> · T <i>tools</i></span></div>
    </Collapsible.Content>
    <Collapsible.Trigger bind:ref={toolsTab} onclick={clickTools} class="doodle-tools-tab" disabled={drawing || pinned} aria-label={pinned ? "Tools pinned open" : "Tools"}><span class="doodle-active-color" style:background={`var(--slop-${marker})`}></span>Tools <ChevronDown size={14} /></Collapsible.Trigger>
    </div>
    </Collapsible.Root>
    </footer>
    <span class="doodle-sr-only" role="status">{notice}</span>
    {#if notice}<div class="doodle-notice" aria-hidden="true">{notice}</div>{/if}
  </main>

  {#snippet exportView()}
    <div class="doodle-export" style:aspect-ratio={`${board.width} / ${board.height}`}><svg xmlns="http://www.w3.org/2000/svg" viewBox={`0 0 ${board.width} ${board.height}`} role="img" aria-label="Doodle Board artwork"><rect width={board.width} height={board.height} fill="var(--slop-board)" />{#each doc.current.strokes as stroke (stroke.$id)}<path d={stroke.geometry} fill={stroke.color} />{/each}</svg></div>
  {/snippet}
  {#snippet icon()}
    <div class="doodle-icon"><div class="doodle-icon-board"><svg viewBox="0 0 240 200" aria-hidden="true"><path d="M30 130Q45 24 79 75T132 60Q188 13 157 108T212 120" fill="none" stroke="var(--slop-blue)" stroke-width="16" stroke-linecap="round" /><path d="m176 32 8-20m9 29 23-5m-27 19 15 15" fill="none" stroke="var(--slop-coral)" stroke-width="8" stroke-linecap="round" /></svg></div><div class="doodle-icon-tray"><i></i><i></i><i></i></div></div>
  {/snippet}
</Slop>
