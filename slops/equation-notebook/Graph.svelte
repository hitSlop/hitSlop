<script lang="ts">
  // The live graph: a JSXGraph board built from the saved line, its parameters and its
  // window, and freed on unmount. Nothing about the board is saved.
  import { onMount } from "svelte";
  import ZoomIn from "@lucide/svelte/icons/zoom-in";
  import ZoomOut from "@lucide/svelte/icons/zoom-out";
  import Crosshair from "@lucide/svelte/icons/crosshair";
  import type { Params, Plot } from "./latex";
  import { defaultWindow, tickStep, yRange, zoomedWindow } from "./notebook";

  let { plot, params, xMin, xMax, label, size = "inline", onwindow }: {
    plot: Plot | null;
    params: Params;
    xMin: number;
    xMax: number;
    label: string;
    size?: "inline" | "large";
    onwindow: (xMin: number, xMax: number) => void;
  } = $props();

  type Library = typeof import("jsxgraph");
  type Board = import("jsxgraph").Board;
  type Point = import("jsxgraph").Point;
  let box: HTMLDivElement;
  let frame: HTMLDivElement;
  let board = $state.raw<Board>();
  let trace: Point | undefined;
  let traceX = $state<number | null>(null);
  let evaluate: (x: number) => number = () => NaN;
  let free: (() => void) | undefined;

  const ok = $derived(!!plot?.ok);
  const traceY = $derived(traceX === null || !plot?.ok ? NaN : plot.evaluate(traceX, params));
  const format = (v: number) => (Number.isFinite(v) ? (Math.abs(v) < 1e-9 ? 0 : v).toFixed(Math.abs(v) >= 100 ? 1 : 2).replace("-", "−") : "undefined");

  function colors() {
    const css = getComputedStyle(frame);
    const read = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
    return { ink: read("--slop-ink", "#1e2b23"), muted: read("--slop-muted", "#55685b"), grid: read("--slop-grid", "#cddfc0"), curve: read("--slop-curve", "#c2412d"), paper: read("--slop-plotPaper", "#fbfcf6") };
  }

  function build(JXG: Library) {
    free?.();
    const c = colors();
    const [yMin, yMax] = yRange(plot, params, xMin, xMax);
    const axis = (anchorX: string, anchorY: string, offset: [number, number]) => ({
      strokeColor: c.ink, strokeWidth: 1.25, highlight: false, name: "", withLabel: false,
      ticks: {
        strokeColor: c.grid, strokeOpacity: 1, highlight: false, minorTicks: 0, insertTicks: false,
        label: { strokeColor: c.muted, fontSize: 11, display: "internal", highlight: false, anchorX, anchorY, offset, cssStyle: "font-variant-numeric: tabular-nums" },
      },
    });
    const created = JXG.JSXGraph.initBoard(box, {
      boundingbox: [xMin, yMax, xMax, yMin], keepAspectRatio: false, axis: true,
      showNavigation: false, showCopyright: false, showInfobox: false, showFullscreen: false, showScreenshot: false,
      pan: { enabled: false }, zoom: { enabled: false, wheel: false }, browserPan: false,
      keyboard: { enabled: false }, registerEvents: true,
      defaultAxes: { x: axis("middle", "top", [0, -4]), y: axis("right", "middle", [-6, 0]) },
    } as never);
    // A parametric curve with plain functions: "functiongraph" builds its x(t) from the
    // string "x" through JessieCode, which needs eval and is blocked by the page's CSP.
    created.create("curve", [(t: number) => t, (t: number) => evaluate(t), () => created.getBoundingBox()[0]!, () => created.getBoundingBox()[2]!], {
      strokeColor: c.curve, strokeWidth: 2.6, highlight: false, fixed: true,
    });
    trace = created.create("point", [() => traceX ?? 0, () => evaluate(traceX ?? 0)], {
      name: "", withLabel: false, size: 4, fillColor: c.curve, strokeColor: c.paper, strokeWidth: 2,
      fixed: true, highlight: false, showInfobox: false, visible: false,
    }) as Point;
    board = created;
    free = () => { JXG.JSXGraph.freeBoard(created); board = undefined; trace = undefined; free = undefined; };
  }

  $effect(() => {
    const p = params, current = plot, a = xMin, b = xMax;
    evaluate = current?.ok ? (x) => current.evaluate(x, p) : () => NaN;
    if (!board) return;
    const [lo, hi] = yRange(current, p, a, b);
    board.setBoundingBox([a, hi, b, lo], false);
    // One labelled tick per ~64px across and ~34px down, at 1, 2 or 5 × 10ⁿ.
    const axes = (board as unknown as { defaultAxes: Record<"x" | "y", { defaultTicks: { setAttribute(a: object): void } }> }).defaultAxes;
    axes.x.defaultTicks.setAttribute({ ticksDistance: tickStep(b - a, Math.max(2, board.canvasWidth / 64)) });
    axes.y.defaultTicks.setAttribute({ ticksDistance: tickStep(hi - lo, Math.max(2, board.canvasHeight / 34)) });
    board.update();
  });

  $effect(() => {
    const visible = traceX !== null && ok && Number.isFinite(traceY);
    if (!board || !trace) return;
    trace.setAttribute({ visible });
    board.update();
  });

  onMount(() => {
    let alive = true;
    let library: Library | undefined;
    // Rebuilds with the new colors when the theme panel changes them.
    const theme = new MutationObserver(() => { if (library) build(library); });
    // Loaded on first use: the module is in the bundle, but evaluates only when a graph shows.
    void import("jsxgraph").then((module) => {
      if (!alive) return;
      const JXG = ((module as { default?: Library }).default ?? module) as Library;
      library = JXG;
      build(JXG);
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "class", "data-theme"] });
    });
    return () => { alive = false; theme.disconnect(); free?.(); };
  });

  function point(event: PointerEvent) {
    if (!board || !ok) return;
    const [x] = board.getUsrCoordsOfMouse(event);
    traceX = Math.max(xMin, Math.min(xMax, x!));
  }
  function key(event: KeyboardEvent) {
    const step = (xMax - xMin) / 40;
    const at = traceX ?? (xMin + xMax) / 2;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") traceX = Math.max(xMin, Math.min(xMax, at + (event.key === "ArrowLeft" ? -step : step)));
    else if (event.key === "+" || event.key === "=") zoom(0.5);
    else if (event.key === "-" || event.key === "_") zoom(2);
    else if (event.key === "0") reset();
    else return;
    event.preventDefault();
  }
  const zoom = (factor: number) => onwindow(...zoomedWindow(xMin, xMax, factor));
  const reset = () => onwindow(defaultWindow.xMin, defaultWindow.xMax);
</script>

<div class="graph" data-size={size} bind:this={frame}>
  <!-- The board is keyboard operable (trace and zoom), which these checks can't see. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="graph-board"
    bind:this={box}
    tabindex="0"
    role="application"
    aria-roledescription="graph"
    aria-label={`${label}. x from ${format(xMin)} to ${format(xMax)}. Arrow keys trace the curve; plus and minus zoom.`}
    data-cant={!ok || undefined}
    onpointermove={point}
    onpointerleave={() => (traceX = null)}
    onfocus={() => { if (traceX === null && ok) traceX = (xMin + xMax) / 2; }}
    onblur={() => (traceX = null)}
    onkeydown={key}
  ></div>
  {#if plot && !plot.ok}
    <div class="cant-plot" role="status">
      <strong>Can't plot this</strong>
      <span>{plot.reason}</span>
    </div>
  {/if}
  <p class="graph-trace" aria-live="polite" data-show={traceX !== null && ok}>
    {#if traceX !== null && ok}x = {format(traceX)}<span>y = {format(traceY)}</span>{/if}
  </p>
  <div class="graph-zoom" role="group" aria-label="Graph window">
    <button type="button" aria-label="Zoom in" onclick={() => zoom(0.5)}><ZoomIn size={16} /></button>
    <button type="button" aria-label="Zoom out" onclick={() => zoom(2)}><ZoomOut size={16} /></button>
    <button type="button" aria-label="Reset to −10 to 10" onclick={reset}><Crosshair size={16} /></button>
  </div>
</div>
