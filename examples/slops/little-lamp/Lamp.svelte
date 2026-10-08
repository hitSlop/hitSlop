<script lang="ts">
  import { onMount } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { capture } from "hitslop/svelte";
  import { Alignment, Fit, Layout, Rive, RuntimeLoader } from "@rive-app/canvas";
  import wasmUrl from "@rive-app/canvas/rive.wasm?url";
  import sceneUrl from "./assets/little-lamp.riv?url";
  import license from "./assets/rive-license.txt?raw";

  let { on, lookX = 0, lookY = 0, still = false }: { on: boolean; lookX?: number; lookY?: number; still?: boolean } = $props();
  let canvas: HTMLCanvasElement;
  let player = $state.raw<Rive>();
  let visible = $state(document.visibilityState !== "hidden");
  let previousOn: boolean | undefined;
  let remaining = 0;
  let frames = 0;
  let captureTimer: ReturnType<typeof setInterval> | undefined;
  let settle: () => void;
  let fail: (error: Error) => void;
  const ready = new Promise<void>((resolve, reject) => { settle = resolve; fail = reject; });
  // The host waits for the actual Rive pose before capturing PNG/PDF or an icon.
  const stopPreparing = capture.onPrepare(() => ready);

  function wake(seconds: number) {
    remaining = Math.max(remaining, seconds);
    frames = 0;
    if (visible || still) player?.startRendering();
  }

  onMount(() => {
    let disposed = false;
    RuntimeLoader.setWasmUrl(wasmUrl);
    RuntimeLoader.setWasmFallbackUrl(null);
    const rive = new Rive({
      canvas, src: sceneUrl, artboard: "Lamp", stateMachine: "Power", autoBind: true, autoplay: true,
      layout: new Layout({ fit: Fit.Contain, alignment: Alignment.Center }),
      enableRiveAssetCDN: false,
      onLoad: () => {
        if (disposed) return;
        if (!rive.viewModelInstance?.boolean("on") || !rive.viewModelInstance.boolean("animate")) {
          fail(new Error("Little Lamp's Rive scene is missing its power bindings."));
          return;
        }
        rive.resizeDrawingSurfaceToCanvas();
        player = rive;
        if (still) {
          // Hidden WKWebView capture pages throttle requestAnimationFrame.
          // The public drawFrame API also advances data bindings synchronously.
          captureTimer = setInterval(() => { rive.stopRendering(); rive.drawFrame(); }, 16);
        }
      },
      onLoadError: event => { if (!disposed) fail(new Error(`Could not load Little Lamp: ${event.data}`)); },
      onAdvance: event => {
        remaining -= Number(event.data);
        if (++frames >= 4) {
          if (captureTimer) { clearInterval(captureTimer); captureTimer = undefined; }
          settle();
        }
        // Awake idle/blinks live inside Rive. Hidden, sleeping and reduced-motion lamps rest.
        if (frames >= 4 && (still || (remaining <= 0 && (!on || prefersReducedMotion.current || !visible))))
          // Rive schedules its next frame after onAdvance returns.
          queueMicrotask(() => { if (!disposed) rive.stopRendering(); });
      },
    });
    const observer = new ResizeObserver(() => {
      if (player) { rive.resizeDrawingSurfaceToCanvas(); wake(.1); }
    });
    observer.observe(canvas);
    return () => { disposed = true; stopPreparing(); clearInterval(captureTimer); observer.disconnect(); rive.cleanup(); };
  });

  $effect(() => {
    if (!player) return;
    const animate = previousOn !== undefined && previousOn !== on && !still && visible && !prefersReducedMotion.current;
    previousOn = on;
    player.viewModelInstance!.boolean("animate")!.value = animate;
    player.viewModelInstance!.boolean("on")!.value = on;
    if (visible || still) wake(animate ? 1.6 : .1);
    else player.stopRendering();
  });
  $effect(() => {
    if (!player) return;
    const track = on && !still && !prefersReducedMotion.current;
    player.viewModelInstance!.number("lookX")!.value = track ? lookX * 18 : 0;
    player.viewModelInstance!.number("lookY")!.value = track ? lookY * 9 : 0;
    player.viewModelInstance!.number("tilt")!.value = track ? lookX * .14 : 0;
    wake(.1);
  });
</script>

<svelte:document onvisibilitychange={() => { visible = document.visibilityState !== "hidden"; previousOn = undefined; }} />
<div class="lamp-stage" data-runtime-license={license}>
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
</div>
